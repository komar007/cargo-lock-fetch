mod batches;
mod cli;

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::OpenOptions,
    io::Write as _,
    iter::once,
    path::Path,
    process::{Command, Stdio},
    str::FromStr as _,
};

use anyhow::{Context, anyhow};
use cargo_lock::{
    Lockfile, Name, Package, SourceId, Version,
    package::{GitReference, SourceKind},
};
use clap::{CommandFactory, Parser as _, error::ErrorKind};
use itertools::{Either, Itertools as _};
use log::{error, info, warn};

use cli::{CargoLockPrefetch, CargoLockPrefetchCli, Cli};

fn main() {
    let cli = Cli::parse();

    let CargoLockPrefetch::LockPrefetch(sub) = cli.subcommand;
    if let Err(error) = run(sub) {
        CargoLockPrefetchCli::command()
            .error(ErrorKind::Io, format!("{error:?}"))
            .exit()
    }
}

fn run(cli: CargoLockPrefetchCli) -> Result<(), anyhow::Error> {
    env_logger::init();

    let lockfile = Lockfile::load(&cli.lockfile_path)
        .with_context(|| format!("could not load lock file {}", &cli.lockfile_path))?;

    let dir = temp_dir::TempDir::new()?;
    run_cargo(&dir, "init", [".", "--name", "fake", "--vcs", "none"])
        .context("failed to create main project")?;

    let (packages, local): (Vec<_>, Vec<_>) = lockfile.packages.into_iter().partition_map(|p| {
        if p.source.is_some() {
            Either::Left((p.name.clone(), p))
        } else {
            Either::Right(p)
        }
    });
    if local.len() > 1 {
        warn!(crates:? = local; "a crate other than root crate has no source");
    }

    let mut registries = BTreeMap::new();
    let batches = batches::into_batches(packages).collect_vec();
    let batch_names = batches
        .into_iter()
        .enumerate()
        .map(|(i, batch)| -> Result<_, anyhow::Error> {
            let batch_no = i + 1;
            let batch_name = format!("batch{batch_no}");
            run_cargo(
                &dir,
                "init",
                [&batch_name, "--name", &batch_name, "--vcs", "none"],
            )
            .with_context(|| format!("failed to create sub-crate for batch {batch_no}"))?;
            add_packages(
                dir.child(&batch_name),
                batch.into_iter().map(|p| Dependency::Real(Box::new(p))),
                &mut registries,
            )
            .with_context(|| format!("failed to add packages for batch {batch_no}"))?;
            Ok(batch_name)
        })
        .collect::<Result<Vec<_>, _>>()?;
    add_packages(
        &dir,
        batch_names.into_iter().map(Dependency::BatchSubCrate),
        &mut registries,
    )
    .context("failed to add sub-crates as dependencies")?;
    run_cargo(&dir, "fetch", [] as [&str; 0]).context("failed to fetch packages")?;
    if let Some(vendor_dir) = cli.vendor_dir {
        let absolute_path = std::env::current_dir()
            .context("Could not determine current directory")?
            .join(vendor_dir);
        let absolute_path = absolute_path.to_str().ok_or_else(|| {
            anyhow!("cannot use path {absolute_path:?} as cargo argument: not utf8")
        })?;
        run_cargo(&dir, "vendor", ["--versioned-dirs", absolute_path])
            .context("failed to vendor packages")?;
    }
    Ok(())
}

fn add_packages(
    dir: impl AsRef<Path>,
    deps: impl IntoIterator<Item = Dependency>,
    registries: &mut BTreeMap<String, String>,
) -> Result<(), anyhow::Error> {
    let deps = deps
        .into_iter()
        .map(|dep| match dep {
            Dependency::Real(p) => p,
            Dependency::BatchSubCrate(name) => Box::new(Package {
                name: Name::from_str(&name).expect("sub-crate's name should be correct"),
                version: Version::new(0, 0, 0),
                source: None,
                checksum: None,
                dependencies: vec![],
                replace: None,
            }),
        })
        .collect_vec();

    let entries: Vec<_> = deps.iter().map(|p| -> Result<_, anyhow::Error> {
        let name = p.name.as_str();
        match &p.source {
            // Our own dummy sub-crate for a batch of crates, because original pakaages without
            // source are filtered out just after parsing Cargo.toml.
            None => Ok(format!(r#"{name} = {{ path = "{path}" }}"#, path = p.name)),
            // Any other original dependency not from default registry
            Some(source) => {
                match source_to_dependency_entry(p.name.as_str(), source, &p.version.to_string(), registries) {
                    Ok(args) => Ok(args),
                    Err(error @ SourceError::Unsupported(_)) => {
                        error!(error:err, dependency_crate:serde = p; "unsupported crate source");
                        Err(error).with_context(|| format!("failed to add crate {name}"))?
                    }
                }
            }
        }
    }).try_collect()?;
    write_registries(&dir, registries).context("Failed to write registries")?;
    write_dependencies(&dir, &entries).context("Failed to write dependencies")?;
    Ok(())
}

fn write_registries(
    dir: impl AsRef<Path>,
    registries: &BTreeMap<String, String>,
) -> Result<(), anyhow::Error> {
    let registries_section = once("[registries]".to_string())
        .chain(
            registries
                .iter()
                .map(|(url, name)| format!("{name} = {{ index = \"{url}\" }}")),
        )
        .join("\n");
    let _ = std::fs::create_dir(dir.as_ref().join(".cargo"));
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(dir.as_ref().join(".cargo/config.toml"))
        .with_context(|| {
            format!(
                "Failed to open .cargo/config.toml in {:?}",
                dir.as_ref().as_os_str()
            )
        })?;
    file.write_all(registries_section.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .with_context(|| {
            format!(
                "Failed to write .cargo/config.toml in {:?}",
                dir.as_ref().as_os_str()
            )
        })
}

fn write_dependencies(dir: impl AsRef<Path>, entries: &[String]) -> Result<(), anyhow::Error> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(dir.as_ref().join("Cargo.toml"))
        .with_context(|| format!("Failed to open Cargo.toml {:?}", dir.as_ref().as_os_str()))?;
    file.write_all(entries.join("\n").as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .with_context(|| {
            format!(
                "Failed to append to Cargo.toml in {:?}",
                dir.as_ref().as_os_str()
            )
        })
}

fn source_to_dependency_entry(
    name: &str,
    source: &SourceId,
    version: &str,
    registries: &mut BTreeMap<String, String>,
) -> Result<String, SourceError> {
    let uri = source.url().as_str();
    let args = match source.kind() {
        SourceKind::Git(git_reference) => {
            let (ref_type, ref_val) = match (source.precise(), git_reference) {
                (Some(precise), _) => ("rev", precise),
                (None, GitReference::Tag(t)) => ("tag", t.as_str()),
                (None, GitReference::Branch(branch)) => {
                    warn!(
                        name, source:serde, branch;
                        "adding crate with no precise rev and branch source, this is not reproducible!"
                    );
                    ("branch", uri)
                }
                (None, GitReference::Rev(r)) => ("rev", r.as_str()),
            };
            format!(
                r#"{name} = {{ git = "{uri}", {ref_type} = "{ref_val}", default-features = false }}"#
            )
        }
        SourceKind::Path => format!(r#"{name} = {{ path = "{uri}", default-features = false }}"#),
        SourceKind::Registry | SourceKind::SparseRegistry => {
            let num = registries.len() + 1;
            let registry_uri = [
                (if *source.kind() == SourceKind::Registry {
                    "registry"
                } else {
                    "sparse"
                }),
                uri,
            ]
            .join("+");
            let reg = registries
                .entry(registry_uri)
                .or_insert_with(|| format!("reg{num}"));
            format!(
                r#"{name} = {{ version = "={version}", registry = "{reg}", default-features = false }}"#
            )
        }
        kind => return Err(SourceError::Unsupported(kind.clone())),
    };
    Ok(args)
}

#[derive(thiserror::Error, Debug)]
enum SourceError {
    #[error("unsupported source {0:?}")]
    Unsupported(SourceKind),
}

enum Dependency {
    Real(Box<Package>),
    BatchSubCrate(String),
}

fn run_cargo<S>(
    cwd: impl AsRef<Path>,
    cargo_cmd: &str,
    args: impl IntoIterator<Item = S>,
) -> Result<(), anyhow::Error>
where
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("cargo");
    let cmd = cmd
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .arg(cargo_cmd)
        .args(args)
        .current_dir(cwd);
    info!(cmd:?; "running cargo");
    let output = cmd.output().context("failed to invoke cargo")?;
    if !output.status.success() {
        let err =
            String::from_utf8(output.stderr).context("cargo returned non-utf8 error output")?;
        Err(std::io::Error::other(format!(
            "failed to run cargo {cargo_cmd}:\n{err}"
        )))?
    }
    Ok(())
}
