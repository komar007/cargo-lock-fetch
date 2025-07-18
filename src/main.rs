use std::{
    collections::BTreeMap,
    ffi::OsStr,
    iter::once,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Context;
use cargo_lock::{Lockfile, Name, Version};
use itertools::{Either, Itertools};
use log::info;

fn main() -> Result<(), anyhow::Error> {
    let lockfile = Lockfile::load("Cargo.lock").unwrap();

    let dir = temp_dir::TempDir::new()?;
    run_cargo(&dir, ["init", ".", "--name", "fake", "--vcs", "none"])
        .context("failed to create main project")?;

    let mut packages: Vec<_> = lockfile
        .packages
        .into_iter()
        .filter(|p| p.source.is_some())
        .collect();
    let mut batches = vec![];
    let mut batch_no = 1;
    while !packages.is_empty() {
        let mut batch_to_add = BTreeMap::new();
        packages = packages
            .into_iter()
            .filter_map(|p| batch_to_add.insert(p.name.clone(), p))
            .collect();
        let batch_name = format!("batch{batch_no}");
        run_cargo(
            &dir,
            ["init", &batch_name, "--name", &batch_name, "--vcs", "none"],
        )
        .with_context(|| format!("failed to create sub-crate for batch {batch_no}"))?;
        add_packages(
            dir.child(&batch_name),
            batch_to_add.values().map(|p| Dependency::External {
                name: &p.name,
                version: &p.version,
            }),
        )
        .with_context(|| format!("failed to run batch {batch_no}"))?;
        batch_no += 1;
        batches.push(batch_name);
    }
    add_packages(
        &dir,
        batches.iter().map(|batch| Dependency::Internal {
            name: batch,
            path: batch,
        }),
    )
    .context("failed to add sub-crate as dependency")?;
    run_cargo(&dir, ["fetch"]).context("failed to fetch packages")?;
    Ok(())
}

fn add_packages<'d>(
    dir: impl AsRef<Path>,
    deps: impl IntoIterator<Item = Dependency<'d>>,
) -> Result<(), anyhow::Error> {
    let (external, internal): (Vec<_>, Vec<_>) = deps.into_iter().partition_map(|dep| match dep {
        Dependency::External { name, version } => Either::Left(format!("{name}@={version}")),
        Dependency::Internal { name, path } => Either::Right((name, path)),
    });

    if !external.is_empty() {
        let batch_add_args = once("add")
            .chain(once("--no-default-features"))
            .map(String::from)
            .chain(external);
        run_cargo(&dir, batch_add_args)
            .context("failed to add a batch of external packages to cargo project")?;
    }
    for (name, path) in internal {
        run_cargo(&dir, ["add", name, "--path", path])
            .context("failed to add a batch sub-crate to cargo project")?;
    }
    Ok(())
}

enum Dependency<'a> {
    External {
        name: &'a Name,
        version: &'a Version,
    },
    Internal {
        name: &'a str,
        path: &'a str,
    },
}

fn run_cargo<S>(
    cwd: impl AsRef<Path>,
    args: impl IntoIterator<Item = S>,
) -> Result<(), anyhow::Error>
where
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("cargo");
    let cmd = cmd
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .args(args)
        .current_dir(cwd);
    info!(cmd:?; "running cargo");
    let output = cmd.output().context("failed to run cargo")?;
    if !output.status.success() {
        let err =
            String::from_utf8(output.stderr).context("cargo returned non-utf8 error output")?;
        Err(std::io::Error::other(format!("cargo failed: {err}")))?
    }
    Ok(())
}
