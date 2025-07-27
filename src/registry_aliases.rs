use std::collections::BTreeMap;

/// Provide distinct aliases for cargo registry URIs to be used in .cargo/config.toml.
pub struct RegistryAliases(BTreeMap<String, String>);

impl RegistryAliases {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Get alias for a cargo registry.
    ///
    /// Equal aliases are always returned for equal URIs. Distinct aliases are always returned for
    /// distinct URIs.
    pub fn get_alias(&mut self, uri: String) -> &str {
        let num = self.0.len() + 1;
        self.0.entry(uri).or_insert_with(|| format!("reg{num}"))
    }

    /// Iterate all aliases as (alias, uri) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(uri, alias)| (alias.as_ref(), uri.as_ref()))
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools as _;

    use super::RegistryAliases;

    #[test]
    fn aliases_correctly_and_enumerates() {
        let mut r = RegistryAliases::new();

        let abc1 = r.get_alias("abc".to_string()).to_owned();
        let def1 = r.get_alias("def".to_string()).to_owned();
        let ghi1 = r.get_alias("ghi".to_string()).to_owned();
        let ghi2 = r.get_alias("ghi".to_string()).to_owned();
        let def2 = r.get_alias("def".to_string()).to_owned();

        assert_ne!(abc1, def1);
        assert_ne!(abc1, ghi1);
        assert_ne!(def1, ghi1);

        assert_eq!(ghi1, ghi2);
        assert_eq!(def1, def2);

        assert_eq!(
            r.iter().map(|(_, u)| u).sorted().collect_vec(),
            vec!["abc", "def", "ghi",]
        )
    }
}
