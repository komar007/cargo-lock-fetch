use std::{fs::OpenOptions, io::Write as _, iter::once, path::Path};

use anyhow::Context;

use crate::registry_aliases::RegistryAliases;

pub fn write_registries(
    dir: impl AsRef<Path>,
    registries: &RegistryAliases,
) -> Result<(), anyhow::Error> {
    use toml_edit::{DocumentMut, Table};

    let registries = Table::from_iter(
        registries
            .iter()
            .map(|(alias, uri)| (alias, Table::from_iter(once(("index", uri))))),
    );
    let config: DocumentMut = Table::from_iter(once(("registries", registries))).into();

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
    file.write_all(config.to_string().as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .with_context(|| {
            format!(
                "Failed to write .cargo/config.toml in {:?}",
                dir.as_ref().as_os_str()
            )
        })
}
