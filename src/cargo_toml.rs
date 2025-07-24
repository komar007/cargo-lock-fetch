use std::{
    fs::OpenOptions,
    io::{Read, Write as _},
    path::Path,
    str::FromStr as _,
};

use anyhow::Context;
use toml_edit::DocumentMut;

pub fn write_dependencies(
    dir: impl AsRef<Path>,
    entries: Vec<(&str, toml_edit::Table)>,
) -> Result<(), anyhow::Error> {
    use toml_edit::Table;

    let mut ro = OpenOptions::new()
        .read(true)
        .open(dir.as_ref().join("Cargo.toml"))
        .with_context(|| format!("Failed to open Cargo.toml {:?}", dir.as_ref().as_os_str()))?;
    let mut cargo = String::new();
    ro.read_to_string(&mut cargo).with_context(|| {
        format!(
            "Could not read from Cargo.toml in {:?}",
            dir.as_ref().as_os_str()
        )
    })?;

    let mut cargo =
        DocumentMut::from_str(&cargo).expect("Cargo.toml created by cargo should be valid TOML");
    cargo["dependencies"] = Table::from_iter(entries).into();

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(dir.as_ref().join("Cargo.toml"))
        .with_context(|| {
            format!(
                "Failed to open Cargo.toml in {:?}",
                dir.as_ref().as_os_str()
            )
        })?;
    file.write_all(cargo.to_string().as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .with_context(|| {
            format!(
                "Failed to append to Cargo.toml in {:?}",
                dir.as_ref().as_os_str()
            )
        })
}
