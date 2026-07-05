use std::{iter::once, path::Path, str::FromStr as _};

use anyhow::Context as _;
use cargo_lock::{Dependency, Lockfile, Name, Package, ResolveVersion, Version};
use itertools::Itertools as _;

/// Build a Cargo.lock for the generated fake project.
///
/// Versions already recorded in a workspace's Cargo.lock are exempt from cargo's yank
/// filter, so shipping this lockfile lets `cargo fetch`/`cargo vendor` download yanked
/// versions pinned by the original lockfile. The lockfile does not have to match fresh
/// resolution exactly: cargo may rewrite it around feature-dependent edges, but keeps
/// the locked versions, which the manifests pin as `=version` anyway.
pub fn synthesize(version: ResolveVersion, batches: &[(String, Vec<Package>)]) -> Lockfile {
    let path_package = |name: &str, dependencies: Vec<Dependency>| Package {
        name: Name::from_str(name).expect("generated crate name should be valid"),
        // Matches the version `cargo init` gives the generated manifests; a mismatch
        // would only make cargo rewrite the entry on the first (non-frozen) resolution.
        version: Version::new(0, 1, 0),
        source: None,
        checksum: None,
        dependencies,
        replace: None,
    };

    let batch_packages = batches
        .iter()
        .map(|(name, packages)| path_package(name, packages.iter().map(Dependency::from).collect()))
        .collect_vec();
    let fake = path_package(
        "fake",
        batch_packages.iter().map(Dependency::from).collect(),
    );

    let packages = batches
        .iter()
        .flat_map(|(_, packages)| packages.iter().cloned())
        .chain(batch_packages)
        .chain(once(fake))
        .collect();

    Lockfile {
        version,
        packages,
        root: None,
        metadata: Default::default(),
        patch: Default::default(),
    }
}

/// Write the lockfile next to the generated project's root Cargo.toml.
pub fn write_lockfile(dir: impl AsRef<Path>, lockfile: &Lockfile) -> Result<(), anyhow::Error> {
    std::fs::write(dir.as_ref().join("Cargo.lock"), lockfile.to_string()).with_context(|| {
        format!(
            "Failed to write Cargo.lock in {:?}",
            dir.as_ref().as_os_str()
        )
    })
}

#[cfg(test)]
mod test {
    use std::str::FromStr as _;

    use cargo_lock::{Lockfile, Package, ResolveVersion};
    use itertools::Itertools as _;

    use super::synthesize;

    const ORIGINAL_LOCKFILE: &str = r#"
version = 4

[[package]]
name = "core2"
version = "0.4.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "b49ba7ef1ad6107f8824dbe97de947cbaac53c44e7f9756a1fba0d37c1eec505"
dependencies = [
 "memchr",
]

[[package]]
name = "memchr"
version = "2.8.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "88904434abc2901f197fe8cc55f0445e7ded921dba5911dad2e2b39b48e663c4"

[[package]]
name = "myproject"
version = "1.0.0"
dependencies = [
 "core2",
 "syn 1.0.109",
 "syn 2.0.118",
 "uses-old-syn",
]

[[package]]
name = "syn"
version = "1.0.109"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"

[[package]]
name = "syn"
version = "2.0.118"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"

[[package]]
name = "uses-old-syn"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
dependencies = [
 "syn 1.0.109",
]
"#;

    /// Non-local packages of the fixture, batched exactly as `cargo_lock_fetch::main` does.
    fn named_batches() -> Vec<(String, Vec<Package>)> {
        let lockfile = Lockfile::from_str(ORIGINAL_LOCKFILE).expect("fixture should parse");
        let packages = lockfile
            .packages
            .into_iter()
            .filter(|p| p.source.is_some())
            .map(|p| (p.name.clone(), p))
            .collect_vec();
        crate::batches::into_batches(packages)
            .enumerate()
            .map(|(i, batch)| (format!("batch{}", i + 1), batch))
            .collect_vec()
    }

    fn find<'a>(lockfile: &'a Lockfile, name: &str) -> Vec<&'a Package> {
        lockfile
            .packages
            .iter()
            .filter(|p| p.name.as_str() == name)
            .collect_vec()
    }

    #[test]
    fn roundtrips_with_fake_root_depending_on_batches() {
        let batches = named_batches();

        let synthesized = synthesize(ResolveVersion::V4, &batches);
        let reparsed = Lockfile::from_str(&synthesized.to_string())
            .expect("synthesized lockfile should parse");

        assert!(reparsed.root.is_none());
        assert!(reparsed.metadata.is_empty());
        assert!(reparsed.patch.is_empty());
        let [fake] = find(&reparsed, "fake")[..] else {
            panic!("exactly one fake package expected");
        };
        assert!(fake.source.is_none());
        assert!(fake.checksum.is_none());
        assert_eq!(
            fake.dependencies
                .iter()
                .map(|d| d.name.as_str())
                .sorted()
                .collect_vec(),
            batches
                .iter()
                .map(|(name, _)| name.as_str())
                .sorted()
                .collect_vec(),
        );
    }

    #[test]
    fn copies_original_package_entries_verbatim() {
        let batches = named_batches();

        let synthesized = synthesize(ResolveVersion::V4, &batches);
        let reparsed = Lockfile::from_str(&synthesized.to_string())
            .expect("synthesized lockfile should parse");

        let originals = Lockfile::from_str(ORIGINAL_LOCKFILE).expect("fixture should parse");
        for original in originals.packages.iter().filter(|p| p.source.is_some()) {
            assert!(
                reparsed.packages.contains(original),
                "package {} {} should be copied verbatim",
                original.name.as_str(),
                original.version,
            );
        }
        assert!(find(&reparsed, "myproject").is_empty());
    }

    #[test]
    fn batch_entries_disambiguate_duplicate_versions() {
        let batches = named_batches();

        let synthesized = synthesize(ResolveVersion::V4, &batches);
        let reparsed = Lockfile::from_str(&synthesized.to_string())
            .expect("synthesized lockfile should parse");

        assert_eq!(find(&reparsed, "syn").len(), 2);
        let syn_deps = reparsed
            .packages
            .iter()
            .filter(|p| p.source.is_none() && p.name.as_str() != "fake")
            .flat_map(|batch| &batch.dependencies)
            .filter(|d| d.name.as_str() == "syn")
            .map(|d| d.version.to_string())
            .sorted()
            .collect_vec();
        assert_eq!(syn_deps, vec!["1.0.109".to_string(), "2.0.118".to_string()]);
    }

    #[test]
    fn preserves_resolve_version() {
        let batches = named_batches();

        let synthesized = synthesize(ResolveVersion::V3, &batches);
        let reparsed = Lockfile::from_str(&synthesized.to_string())
            .expect("synthesized lockfile should parse");

        assert_eq!(reparsed.version, ResolveVersion::V3);
    }
}
