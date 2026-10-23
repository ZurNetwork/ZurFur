use std::collections::{HashMap, HashSet};

/// The first number a later entry of the same name can get.
const FIRST: u32 = 2;

/// The number that tells an entry apart from the earlier entries of the same name in one
/// viewer's listing, shown as "name (n)"; never below 2.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::Display,
    derive_more::Into,
)]
pub struct NameOrdinal(u32);

/// Why a number is not a [`NameOrdinal`]: it is below 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a name ordinal is 2 or more")]
pub struct NameOrdinalError;

impl TryFrom<u32> for NameOrdinal {
    type Error = NameOrdinalError;

    /// Any number from 2 up.
    fn try_from(number: u32) -> Result<Self, Self::Error> {
        if number < FIRST {
            return Err(NameOrdinalError);
        }
        Ok(Self(number))
    }
}

/// The key under which two names count as the same: for now the exact name, so case and every
/// other difference keep names apart. Comparing NFC forms instead changes only this function.
pub(crate) fn comparison_key(name: &str) -> String {
    name.to_owned()
}

/// The number each entry gets, given each entry's name and its place among entries of the same
/// name, in listing order. The first entry of each group gets none; each later one gets the
/// smallest number, rising through the group, whose "name (n)" key is neither another entry's
/// key nor a key already given.
pub(crate) fn ordinals<T: Ord>(entries: &[(&str, T)]) -> Vec<Option<NameOrdinal>> {
    let keys: Vec<String> = entries
        .iter()
        .map(|(name, _)| comparison_key(name))
        .collect();
    let mut taken: HashSet<String> = keys.iter().cloned().collect();
    let mut group_of = HashMap::<&str, usize>::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        let group = *group_of.entry(key.as_str()).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(index);
    }
    let mut numbered = vec![None; entries.len()];
    for mut group in groups {
        group.sort_by(|left, right| entries[*left].1.cmp(&entries[*right].1));
        let mut next = FIRST;
        for index in group.into_iter().skip(1) {
            let name = entries[index].0;
            let Some((number, key)) = free_number(name, next, &taken) else {
                continue;
            };
            taken.insert(key);
            numbered[index] = Some(NameOrdinal(number));
            next = number.saturating_add(1);
        }
    }
    numbered
}

/// The smallest number from `start` up whose "name (n)" key is not `taken`, with that key.
fn free_number(name: &str, start: u32, taken: &HashSet<String>) -> Option<(u32, String)> {
    (start..=u32::MAX)
        .map(|number| (number, comparison_key(&format!("{name} ({number})"))))
        .find(|(_, key)| !taken.contains(key))
}

#[cfg(test)]
mod tests;
