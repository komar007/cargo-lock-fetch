use std::{
    iter::once,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr as _,
};

use anyhow::{Context, anyhow};
use cargo_lock::{
    Lockfile, Name, Package, SourceId, Version,
    package::{GitReference, SourceKind},
};
use itertools::{Either, Itertools as _};
use log::{error, warn};
use unwrap_infallible::UnwrapInfallible as _;

use crate::batches;
use crate::cargo;
use crate::cargo_config_toml;
use crate::cargo_toml;
use crate::cli::CargoLockFetchCli;
use crate::lockfile_synth;
use crate::registry_aliases::RegistryAliases;

pub fn main(cli: &CargoLockFetchCli) -> Result<ExitCode, anyhow::Error> {
    env_logger::init();

    let lockfile = Lockfile::load(&cli.lockfile_path)
        .with_context(|| format!("could not load lock file {}", cli.lockfile_path))?;
    let resolve_version = lockfile.version;

    let dir: Box<dyn AsRef<Path>> = if let Some(ref dir) = cli.tmp_dir {
        Box::new(PathBuf::from_str(dir).unwrap_infallible()) as _
    } else {
        let mut dir = temp_dir::TempDir::new()?;
        if cli.keep_tmp {
            if !cli.quiet {
                eprintln!(
                    "project directory: {}",
                    dir.as_ref().to_str().expect("temp dir should be utf-8")
                );
            }
            dir = dir.dont_delete_on_drop();
        }
        Box::new(dir) as _
    };

    cargo::run(
        dir.as_ref(),
        "init",
        [".", "--name", "fake", "--vcs", "none"],
        cli.quiet,
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

    let mut registries = RegistryAliases::new();
    let batches = batches::into_batches(packages)
        .enumerate()
        .map(|(i, batch)| (format!("batch{}", i + 1), batch))
        .collect_vec();
    for (batch_name, batch) in &batches {
        cargo::run(
            dir.as_ref(),
            "init",
            [batch_name.as_str(), "--name", batch_name, "--vcs", "none"],
            cli.quiet,
        )
        .with_context(|| format!("failed to create sub-crate for {batch_name}"))?;
        let child = dir.as_ref().as_ref().join(batch_name);
        add_packages(
            child,
            batch.iter().map(|p| Dependency::Real(Box::new(p.clone()))),
            &mut registries,
        )
        .with_context(|| format!("failed to add packages for {batch_name}"))?;
    }
    add_packages(
        dir.as_ref(),
        batches
            .iter()
            .map(|(batch_name, _)| Dependency::BatchSubCrate(batch_name.clone())),
        &mut registries,
    )
    .context("failed to add sub-crates as dependencies")?;

    // Written after the manifests so that cargo sees a complete workspace: versions
    // recorded in a Cargo.lock are exempt from cargo's yank filter, which lockfiles
    // containing yanked versions rely on.
    let synthesized = lockfile_synth::synthesize(resolve_version, &batches);
    lockfile_synth::write_lockfile(dir.as_ref(), &synthesized)
        .context("failed to write synthesized Cargo.lock")?;

    let cargo_status = if let Some(ref vendor_dir) = cli.vendor_dir {
        let absolute_path = std::env::current_dir()
            .context("Could not determine current directory")?
            .join(vendor_dir);
        let absolute_path = absolute_path.to_str().ok_or_else(|| {
            anyhow!("cannot use path {absolute_path:?} as cargo argument: not utf8")
        })?;
        cargo::run_passthrough(
            dir.as_ref(),
            "vendor",
            ["--manifest-path", "Cargo.toml", absolute_path]
                .into_iter()
                .chain(cli.cargo_args.iter().map(AsRef::as_ref)),
            cli.quiet,
        )
        .context("failed to vendor packages")?
    } else {
        cargo::run_passthrough(
            dir.as_ref(),
            "fetch",
            ["--manifest-path", "Cargo.toml"]
                .into_iter()
                .chain(cli.cargo_args.iter().map(AsRef::as_ref)),
            cli.quiet,
        )
        .context("failed to fetch packages")?
    };
    let cargo_code = cargo_status
        .code()
        .map(|c| (c as u8).into())
        .unwrap_or(ExitCode::FAILURE);
    Ok(cargo_code)
}

fn add_packages(
    dir: impl AsRef<Path>,
    deps: impl IntoIterator<Item = Dependency>,
    registries: &mut RegistryAliases,
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
    registries: &mut RegistryAliases,
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
        // The default registry is left implicit so that cargo resolves it exactly like the
        // original project would, honoring the default protocol and any source replacement,
        // and therefore populates the same registry cache.
        SourceKind::Registry | SourceKind::SparseRegistry if source.is_default_registry() => {
            Table::from_iter([("version", v(format!("={version}")))])
        }
        // In .cargo/config.toml, a registry index is either a bare URL (git index) or a
        // "sparse+"-prefixed URL; the "registry+" prefix is Cargo.lock's source-id encoding
        // and is rejected by cargo since 1.96.
        SourceKind::Registry | SourceKind::SparseRegistry => {
            let registry_uri = if *source.kind() == SourceKind::Registry {
                uri.to_string()
            } else {
                format!("sparse+{uri}")
            };
            Table::from_iter([
                ("version", v(format!("={version}"))),
                ("registry", v(registries.get_alias(registry_uri))),
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

#[cfg(test)]
mod test {
    use cargo_lock::SourceId;
    use itertools::Itertools as _;

    use super::source_to_dependency_entry;
    use crate::registry_aliases::RegistryAliases;

    fn entry_and_registries(source_url: &str) -> (toml_edit::Table, Vec<(String, String)>) {
        let source = SourceId::from_url(source_url).expect("source url should parse");
        let mut registries = RegistryAliases::new();
        let entry = source_to_dependency_entry("foo", &source, "1.2.3", &mut registries)
            .expect("registry source should be supported");
        let registries = registries
            .iter()
            .map(|(a, u)| (a.to_owned(), u.to_owned()))
            .collect_vec();
        (entry, registries)
    }

    #[test]
    fn crates_io_dependency_uses_implicit_registry() {
        let (entry, registries) =
            entry_and_registries("registry+https://github.com/rust-lang/crates.io-index");

        assert_eq!(entry["version"].as_str(), Some("=1.2.3"));
        assert!(!entry.contains_key("registry"));
        assert_eq!(registries, vec![]);
    }

    #[test]
    fn sparse_crates_io_dependency_uses_implicit_registry() {
        let (entry, registries) = entry_and_registries("sparse+https://index.crates.io/");

        assert!(!entry.contains_key("registry"));
        assert_eq!(registries, vec![]);
    }

    #[test]
    fn git_registry_index_has_no_protocol_prefix() {
        let (entry, registries) = entry_and_registries("registry+https://example.com/index");

        assert_eq!(
            registries,
            vec![(
                entry["registry"].as_str().unwrap_or_default().to_string(),
                "https://example.com/index".to_string()
            )]
        );
    }

    #[test]
    fn sparse_registry_index_keeps_sparse_prefix() {
        let (entry, registries) = entry_and_registries("sparse+https://example.com/index/");

        assert_eq!(
            registries,
            vec![(
                entry["registry"].as_str().unwrap_or_default().to_string(),
                "sparse+https://example.com/index/".to_string()
            )]
        );
    }
}
