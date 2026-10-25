use std::collections::{BTreeMap, BTreeSet};

pub const MAX_LEVEL: u8 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub level: u8,
    pub start: u32,
    pub end: u32,
}

pub fn max_level(levels: &BTreeMap<u32, u8>) -> u8 {
    levels.values().copied().max().unwrap_or(0)
}

pub fn groups(levels: &BTreeMap<u32, u8>) -> Vec<Group> {
    let mut out = Vec::new();
    for level in 1..=max_level(levels) {
        let mut run: Option<(u32, u32)> = None;
        for (&index, &value) in levels {
            if value < level {
                continue;
            }
            run = match run {
                Some((start, end)) if end + 1 == index => Some((start, index)),
                Some((start, end)) => {
                    out.push(Group { level, start, end });
                    Some((index, index))
                }
                None => Some((index, index)),
            };
        }
        if let Some((start, end)) = run {
            out.push(Group { level, start, end });
        }
    }
    out
}

pub fn group(levels: &mut BTreeMap<u32, u8>, start: u32, end: u32) {
    for index in start..=end {
        let level = levels.entry(index).or_insert(0);
        *level = (*level + 1).min(MAX_LEVEL);
    }
}

pub fn ungroup(levels: &mut BTreeMap<u32, u8>, start: u32, end: u32) -> bool {
    let mut changed = false;
    for index in start..=end {
        if let Some(level) = levels.get_mut(&index) {
            *level -= 1;
            changed = true;
            if *level == 0 {
                levels.remove(&index);
            }
        }
    }
    changed
}

pub fn is_collapsed(hidden: &BTreeSet<u32>, group: Group) -> bool {
    (group.start..=group.end).all(|index| hidden.contains(&index))
}

pub fn collapse(hidden: &mut BTreeSet<u32>, collapsed: &mut BTreeSet<u32>, group: Group) {
    hidden.extend(group.start..=group.end);
    collapsed.insert(group.end);
}

pub fn expand(
    hidden: &mut BTreeSet<u32>,
    collapsed: &mut BTreeSet<u32>,
    levels: &BTreeMap<u32, u8>,
    group: Group,
) {
    collapsed.remove(&group.end);
    let closed: Vec<Group> = groups(levels)
        .into_iter()
        .filter(|inner| {
            inner.level > group.level
                && inner.start >= group.start
                && inner.end <= group.end
                && collapsed.contains(&inner.end)
        })
        .collect();
    for index in group.start..=group.end {
        if !closed
            .iter()
            .any(|inner| (inner.start..=inner.end).contains(&index))
        {
            hidden.remove(&index);
        }
    }
}

pub fn show_level(
    hidden: &mut BTreeSet<u32>,
    collapsed: &mut BTreeSet<u32>,
    levels: &BTreeMap<u32, u8>,
    shown: u8,
) {
    for (&index, &level) in levels {
        if level >= shown {
            hidden.insert(index);
        } else {
            hidden.remove(&index);
        }
    }
    collapsed.clear();
    collapsed.extend(
        groups(levels)
            .into_iter()
            .filter(|group| group.level >= shown)
            .map(|group| group.end),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels(pairs: &[(u32, u8)]) -> BTreeMap<u32, u8> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn finds_nested_groups() {
        let mut map = BTreeMap::new();
        group(&mut map, 1, 6);
        group(&mut map, 2, 3);
        group(&mut map, 5, 5);
        assert_eq!(max_level(&map), 2);
        assert_eq!(
            groups(&map),
            [
                Group {
                    level: 1,
                    start: 1,
                    end: 6
                },
                Group {
                    level: 2,
                    start: 2,
                    end: 3
                },
                Group {
                    level: 2,
                    start: 5,
                    end: 5
                },
            ]
        );
        assert!(ungroup(&mut map, 2, 3));
        assert_eq!(
            map,
            levels(&[(1, 1), (2, 1), (3, 1), (4, 1), (5, 2), (6, 1)])
        );
        assert!(!ungroup(&mut map, 9, 9));
    }

    #[test]
    fn collapses_and_expands_keeping_closed_children() {
        let map = levels(&[(1, 1), (2, 2), (3, 2), (4, 1)]);
        let outer = Group {
            level: 1,
            start: 1,
            end: 4,
        };
        let inner = Group {
            level: 2,
            start: 2,
            end: 3,
        };
        let mut hidden = BTreeSet::new();
        let mut closed = BTreeSet::new();
        collapse(&mut hidden, &mut closed, inner);
        collapse(&mut hidden, &mut closed, outer);
        assert!(is_collapsed(&hidden, outer));
        expand(&mut hidden, &mut closed, &map, outer);
        assert_eq!(hidden, [2, 3].into_iter().collect());
        expand(&mut hidden, &mut closed, &map, inner);
        assert!(hidden.is_empty());
        collapse(&mut hidden, &mut closed, outer);
        expand(&mut hidden, &mut closed, &map, outer);
        assert!(hidden.is_empty());
        show_level(&mut hidden, &mut closed, &map, 2);
        assert_eq!(hidden, [2, 3].into_iter().collect());
        assert!(is_collapsed(&hidden, inner));
        show_level(&mut hidden, &mut closed, &map, 1);
        assert_eq!(hidden, [1, 2, 3, 4].into_iter().collect());
        show_level(&mut hidden, &mut closed, &map, 3);
        assert!(hidden.is_empty());
        assert!(closed.is_empty());
    }
}
