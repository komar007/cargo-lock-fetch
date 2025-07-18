//! Batch items so that no single batch contains items in conflict with each other.
//!
//! This solves the problem where multiple versions of the same crate cannot be added to a single
//! Cargo.toml file with `cargo add`. This module can be used to divide them into batches that neve
//! contain more than one version of the same crate.

use std::collections::BTreeMap;

/// Divide input items into batches such that no batch contains two items of the same key.
pub fn into_batches<K, T>(mut items: Vec<(K, T)>) -> impl Iterator<Item = Vec<T>>
where
    K: Ord + Clone,
{
    std::iter::from_fn(move || {
        if items.is_empty() {
            return None;
        }
        let mut batch = BTreeMap::new();
        items = std::mem::take(&mut items)
            .into_iter()
            .filter_map(|(k, item)| batch.insert(k.clone(), item).map(|old| (k, old)))
            .collect();
        Some(batch.into_values().collect())
    })
}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use super::into_batches;

    #[test]
    fn into_batches_empty() {
        let e: Vec<((), ())> = vec![];
        let batches = into_batches(e).collect_vec();
        assert!(batches.is_empty());
    }

    #[test]
    fn into_batches_one() {
        let batches = into_batches(vec![(1, "x")]).collect_vec();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 1);
        assert_eq!(batches[0][0], "x");
    }

    #[test]
    fn into_batches_no_conflicts() {
        let batches = into_batches(vec![
            (1, "a"),
            (2, "b"),
            (1, "c"),
            (3, "d"),
            (2, "e"),
            (4, "f"),
            (2, "g"),
        ])
        .collect_vec();
        assert_eq!(batches.len(), 3);
        batches.iter().all(|batch| {
            ["a", "c"]
                .iter()
                .filter(|x| batch.contains(x))
                .exactly_one()
                .is_ok()
        });
        batches.iter().all(|batch| {
            ["b", "e", "g"]
                .iter()
                .filter(|x| batch.contains(x))
                .exactly_one()
                .is_ok()
        });
    }
}
