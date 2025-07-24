mod batches;
mod cargo_config_toml;
mod cargo_toml;
mod cli;

use std::{
    collections::BTreeMap,
    ffi::OsStr,
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

    let mut dir = temp_dir::TempDir::new()?;

    if cli.keep_tmp {
        eprintln!(
            "project directory: {}",
            dir.as_ref().to_str().expect("temp dir should be utf-8")
        );
        dir = dir.dont_delete_on_drop();
    }

    run_cargo(
        &dir,
        "init",
        [".", "--name", "fake", "--vcs", "none"],
        false,
    )
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
                false,
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
    run_cargo(&dir, "fetch", [] as [&str; 0], false).context("failed to fetch packages")?;
    if let Some(vendor_dir) = cli.vendor_dir {
        let absolute_path = std::env::current_dir()
            .context("Could not determine current directory")?
            .join(vendor_dir);
        let absolute_path = absolute_path.to_str().ok_or_else(|| {
            anyhow!("cannot use path {absolute_path:?} as cargo argument: not utf8")
        })?;
        run_cargo(
            &dir,
            "vendor",
            [absolute_path, "--frozen"]
                .into_iter()
                .chain(cli.versioned_dirs.then_some("--versioned-dirs")),
            true,
        )
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

    let entries = deps.iter().map(|p| {
        let name = p.name.as_str();
        let spec = match &p.source {
            // Our own dummy sub-crate for a batch of crates, because original pakaages without
            // source are filtered out just after parsing Cargo.toml.
            None => Ok(toml_edit::Table::from_iter(once(("path", p.name.as_str())))),
            // Any other original dependency not from default registry
            Some(source) => {
                match source_to_dependency_entry(p.name.as_str(), source, &p.version.to_string(), registries) {
                    Ok(spec) => Ok(spec),
                    Err(error @ SourceError::Unsupported(_)) => {
                        error!(error:err, dependency_crate:serde = p; "unsupported crate source");
                        Err(error).with_context(|| format!("failed to add crate {name}"))?
                    }
                }
            }
        };
        spec.map(|s| (name, s))
    }).try_collect::<_, _, anyhow::Error>()?;
    cargo_config_toml::write_registries(&dir, registries).context("Failed to write registries")?;
    cargo_toml::write_dependencies(&dir, entries).context("Failed to write dependencies")?;
    Ok(())
}

fn source_to_dependency_entry(
    name: &str,
    source: &SourceId,
    version: &str,
    registries: &mut BTreeMap<String, String>,
) -> Result<toml_edit::Table, SourceError> {
    use toml_edit::{Table, value as v};

    let uri = source.url().as_str();
    let mut entry = match source.kind() {
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
            Table::from_iter([("git", v(uri)), (ref_type, v(ref_val))])
        }
        SourceKind::Path => Table::from_iter([("path", v(uri))]),
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
            Table::from_iter([
                ("version", v(format!("={version}"))),
                ("registry", v(&*reg)),
            ])
        }
        kind => return Err(SourceError::Unsupported(kind.clone())),
    };
    entry["default-features"] = v(false);
    Ok(entry)
}

#[derive(thiserror::Error, Debug)]
pub enum SourceError {
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
    inherit_output: bool,
) -> Result<(), anyhow::Error>
where
    S: AsRef<OsStr>,
{
    let (out_cfg, err_cfg) = if inherit_output {
        (Stdio::inherit(), Stdio::inherit())
    } else {
        (Stdio::null(), Stdio::piped())
    };
    let mut cmd = Command::new("cargo");
    let cmd = cmd
        .stdout(out_cfg)
        .stderr(err_cfg)
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
