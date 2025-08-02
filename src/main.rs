mod batches;
mod cargo;
mod cargo_config_toml;
mod cargo_lock_fetch;
mod cargo_toml;
mod cli;
mod registry_aliases;

use std::process::ExitCode;

use clap::{CommandFactory, Parser as _, error::ErrorKind};

use crate::cli::{CargoLockFetch, CargoLockFetchCli, Cli};

shadow_rs::shadow!(build);

fn main() -> ExitCode {
    let cli = Cli::parse();

    let CargoLockFetch::LockFetch(sub) = cli.subcommand;
    let quiet = sub.quiet;
    let sub = match sub.verify() {
        Ok(sub) => sub,
        Err((kind, msg)) => exit_cli_error(quiet, kind, &msg),
    };

    match cargo_lock_fetch::main(&sub) {
        Ok(status) => status,
        Err(error) => {
            exit_cli_error(quiet, ErrorKind::Io, &format!("{error:?}"));
        }
    }
}

fn exit_cli_error(quiet: bool, kind: ErrorKind, msg: &str) -> ! {
    if quiet {
        std::process::exit(2);
    }
    CargoLockFetchCli::command().error(kind, msg).exit()
}
