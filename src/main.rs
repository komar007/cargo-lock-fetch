use std::{
    collections::BTreeMap,
    ffi::OsStr,
    iter::once,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Context;
use cargo_lock::{Lockfile, Package};
use log::info;

fn main() -> Result<(), anyhow::Error> {
    let lockfile = Lockfile::load("Cargo.lock").unwrap();
    let mut packages: Vec<_> = lockfile
        .packages
        .into_iter()
        .filter(|p| p.source.is_some())
        .collect();
    let mut batch = 1;
    while !packages.is_empty() {
        let mut batch_to_add = BTreeMap::new();
        packages = packages
            .into_iter()
            .filter_map(|p| batch_to_add.insert(p.name.clone(), p))
            .collect();
        prefetch(batch_to_add.into_values(), &format!("prefetch-{batch}"))
            .with_context(|| format!("failed to run batch {batch}"))?;
        batch += 1;
    }
    Ok(())
}

fn prefetch(
    packages: impl IntoIterator<Item = Package>,
    project_name: &str,
) -> Result<(), anyhow::Error> {
    let dir = temp_dir::TempDir::new()?;
    run_cargo(&dir, ["init", ".", "--name", project_name])
        .context("failed to init cargo project")?;
    let batch_add_args = once("add".to_string()).chain(
        packages
            .into_iter()
            .map(|p| format!("{}@={}", p.name, p.version)),
    );
    run_cargo(&dir, batch_add_args)
        .context("failed to add a batch of packages to cargo project")?;
    run_cargo(&dir, ["fetch"]).context("failed to fetch packages")?;
    Ok(())
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
