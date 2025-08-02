use clap::error::ErrorKind;
use clap_cargo::style::CLAP_STYLING;
use indoc::indoc;

#[derive(clap::Parser, Debug)]
#[command(
    name = "cargo",
    bin_name = "cargo",
    styles = CLAP_STYLING,
)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: CargoLockFetch,
}

#[derive(clap::Subcommand, Debug)]
pub enum CargoLockFetch {
    LockFetch(CargoLockFetchCli),
}

#[derive(Debug, clap::Parser)]
#[command(
    name = "cargo lock-fetch",
    trailing_var_arg = true,
    version,
    long_version = crate::build::CLAP_LONG_VERSION,
    styles = CLAP_STYLING,
    about = "Fetch crate dependencies from Cargo.lock",
    after_help = indoc! {"
        This cargo plugin fetches and vendors dependencies without accessing any
        Cargo.toml files.
    "}
)]
pub struct CargoLockFetchCli {
    #[arg(
        long,
        value_name = "PATH",
        default_value = "Cargo.lock",
        help = "Path to Cargo.lock"
    )]
    pub lockfile_path: String,

    #[arg(
        name = "vendor",
        value_name = "DIR",
        long,
        id = "vendor",
        help = "Vendor all dependencies for a project locally"
    )]
    pub vendor_dir: Option<String>,

    #[arg(
        long,
        short,
        default_value = "false",
        help = "Do not print any messages, even errors"
    )]
    pub quiet: bool,

    #[arg(
        long,
        default_value = "false",
        help = "Do not remove temporary cargo project's directory, print its name to stderr"
    )]
    pub keep_tmp: bool,

    #[arg(
        value_name = "DIR",
        long,
        help = indoc! {"
            Keep temporary files in <DIR>, the directory must exist, implies --keep-tmp, but
            does not print <DIR> to stderr
        "}
    )]
    pub tmp_dir: Option<String>,

    #[arg(
        help = indoc!{"
            Arguments to pass through to cargo. Cargo fetch/vendor will be called on
            a generated Cargo.toml file - not all options make sense in this context
        "}
    )]
    pub cargo_args: Vec<String>,
}

impl CargoLockFetchCli {
    pub fn verify(self) -> Result<Self, (ErrorKind, String)> {
        if self.keep_tmp && self.tmp_dir.is_some() {
            Err((
                ErrorKind::ArgumentConflict,
                "arguments --keep-tmp and --tmp-dir are mutually exclusive".to_string(),
            ))?;
        }
        Ok(self)
    }
}
