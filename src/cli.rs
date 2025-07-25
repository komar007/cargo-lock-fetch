use clap_cargo::style::CLAP_STYLING;

#[derive(clap::Parser, Debug)]
#[command(
    name = "cargo",
    bin_name = "cargo",
    styles = CLAP_STYLING,
)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: CargoLockPrefetch,
}

#[derive(clap::Subcommand, Debug)]
pub enum CargoLockPrefetch {
    LockPrefetch(CargoLockPrefetchCli),
}

#[derive(Debug, clap::Parser)]
#[command(
    name = "cargo lock-prefetch",
    version,
    styles = CLAP_STYLING,
    about = "Prefetch crate dependencies from Cargo.lock",
    after_help = indoc::indoc! {"
        This cargo plugin prefetches and vendors dependencies without accessing any
        Cargo.toml files.
    "}
)]
pub struct CargoLockPrefetchCli {
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
        requires = "vendor",
        default_value = "false",
        help = "Always include version in subdir name, only valid with --vendor"
    )]
    pub versioned_dirs: bool,

    #[arg(
        long,
        default_value = "false",
        help = "Do not remove temporary cargo project's directory, print its name to stderr"
    )]
    pub keep_tmp: bool,
}
