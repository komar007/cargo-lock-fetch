mod batches;
mod cli;

use std::{
    ffi::OsStr,
    iter::{empty, once},
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

    let batches = batches::into_batches(packages);
    let batch_names = batches
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
            )
            .with_context(|| format!("failed to run batch {batch_no}"))?;
            Ok(batch_name)
        })
        .collect::<Result<Vec<_>, _>>()?;
    add_packages(&dir, batch_names.into_iter().map(Dependency::BatchSubCrate))
        .context("failed to add sub-crate as dependency")?;
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
) -> Result<(), anyhow::Error> {
    let (default_registry, rest): (Vec<_>, Vec<_>) =
        deps.into_iter().partition_map(|dep| match dep {
            Dependency::Real(p) if p.source.as_ref().is_some_and(SourceId::is_default_registry) => {
                Either::Left(format!("{}@={}", p.name, p.version))
            }
            Dependency::Real(p) => Either::Right(p),
            Dependency::BatchSubCrate(name) => Either::Right(Box::new(Package {
                name: Name::from_str(&name).expect("sub-crate's name should be correct"),
                version: Version::new(0, 1, 0),
                source: None,
                checksum: None,
                dependencies: vec![],
                replace: None,
            })),
        });

    if !default_registry.is_empty() {
        let batch_add_args = empty()
            .chain(["--config", "net.git-fetch-with-cli=true"])
            .chain(once("--no-default-features"))
            .map(String::from)
            .chain(default_registry);
        run_cargo(&dir, "add", batch_add_args)
            .context("failed to add a batch of external packages to cargo project")?;
    }
    for p in &rest {
        let name = p.name.as_str();
        let args = match &p.source {
            // Our own dummy sub-crate for a batch of crates, because original pakaages without
            // source are filtered out just after parsing Cargo.toml.
            None => vec![name, "--path", p.name.as_str()],
            // Any other original dependency not from default registry
            Some(source) => match source_to_cargo_add_args(p.name.as_str(), source) {
                Ok(args) => args,
                Err(error @ SourceError::Unsupported(_)) => {
                    error!(error:err, dependency_crate:serde = p; "unsupported crate source");
                    Err(error).with_context(|| format!("failed to add crate {name}"))?
                }
            },
        };
        run_cargo(
            &dir,
            "add",
            empty()
                .chain(["--config", "net.git-fetch-with-cli=true"])
                .chain(once("--no-default-features"))
                .chain(args),
        )
        .context("failed to add a batch sub-crate to cargo prokect")?;
    }
    Ok(())
}

fn source_to_cargo_add_args<'a>(
    name: &'a str,
    source: &'a SourceId,
) -> Result<Vec<&'a str>, SourceError> {
    let uri = source.url().as_str();
    let args = match source.kind() {
        SourceKind::Git(git_reference) => {
            let (ref_name, ref_val) = match (source.precise(), git_reference) {
                (Some(precise), _) => ("--rev", precise),
                (None, GitReference::Tag(t)) => ("--tag", t.as_str()),
                (None, GitReference::Branch(branch)) => {
                    warn!(
                        name, source:serde, branch;
                        "adding crate with no precise rev and branch source, this is not reproducible!"
                    );
                    ("--branch", uri)
                }
                (None, GitReference::Rev(r)) => ("--rev", r.as_str()),
            };
            vec![name, "--git", uri, ref_name, ref_val]
        }
        SourceKind::Path => vec![name, "--path", uri],
        SourceKind::Registry | SourceKind::SparseRegistry => vec![name, "--registry", uri],
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
