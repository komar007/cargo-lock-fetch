use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, ExitStatus, Stdio},
};

use anyhow::{Context, anyhow};
use log::{debug, info, warn};

pub fn run<S>(
    cwd: impl AsRef<Path>,
    cargo_cmd: &str,
    args: impl IntoIterator<Item = S>,
    quiet: bool,
) -> Result<(), anyhow::Error>
where
    S: AsRef<OsStr>,
{
    let status = run_cargo_impl(cwd.as_ref(), cargo_cmd, args, false, quiet)?;
    assert!(status.success());
    Ok(())
}

pub fn run_passthrough<S>(
    cwd: impl AsRef<Path>,
    cargo_cmd: &str,
    args: impl IntoIterator<Item = S>,
    quiet: bool,
) -> Result<ExitStatus, anyhow::Error>
where
    S: AsRef<OsStr>,
{
    run_cargo_impl(cwd.as_ref(), cargo_cmd, args, true, quiet)
}

fn run_cargo_impl<S>(
    cwd: &Path,
    cargo_cmd: &str,
    args: impl IntoIterator<Item = S>,
    passthrough: bool,
    quiet: bool,
) -> Result<ExitStatus, anyhow::Error>
where
    S: AsRef<OsStr>,
{
    let c = std::env::var("CARGO");
    let cargo_bin = c
        .as_ref()
        .map(AsRef::as_ref)
        .inspect_err(|error| {
            warn!(error:err = **error; "could not retrieve , calling cargo directly");
        })
        .inspect(|cargo| {
            debug!(cargo; "calling ");
        })
        .unwrap_or("cargo");
    if !cwd.is_dir() {
        let dir = cwd.as_os_str();
        Err(if cwd.exists() {
            anyhow!("{:?} is not a directory", dir)
        } else {
            anyhow!("{:?} does not exist", dir)
        })?
    }
    let (out_cfg, err_cfg) = if passthrough {
        (Stdio::inherit(), Stdio::inherit())
    } else {
        (Stdio::null(), Stdio::piped())
    };
    let mut cmd = Command::new(cargo_bin);
    let cmd = cmd
        .stdout(out_cfg)
        .stderr(err_cfg)
        .arg(cargo_cmd)
        .args(args)
        .current_dir(cwd);
    if quiet {
        cmd.arg("-q");
    }
    info!(cmd:?; "running cargo");
    let output = cmd
        .output()
        .with_context(|| format!("failed to invoke cargo {cargo_cmd}"))?;
    if !passthrough && !output.status.success() {
        let err =
            String::from_utf8(output.stderr).context("cargo returned non-utf8 error output")?;
        Err(std::io::Error::other(format!(
            "\"cargo {cargo_cmd}\" returned error:\n{err}"
        )))?
    }
    Ok(output.status)
}
