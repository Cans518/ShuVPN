use std::cmp::Ordering;

struct Version {
    core: Vec<i64>,
    pre: Vec<String>,
}

fn parse(raw: &str) -> Version {
    let text = raw.trim();
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    let text = text.split('+').next().unwrap_or("");
    let (core, pre) = text.split_once('-').unwrap_or((text, ""));
    Version {
        core: core
            .split('.')
            .map(|part| part.trim().parse().unwrap_or(0))
            .collect(),
        pre: if pre.is_empty() {
            vec![]
        } else {
            pre.split('.').map(|part| part.trim().to_owned()).collect()
        },
    }
}

pub(super) fn compare(left: &str, right: &str) -> Ordering {
    let left = parse(left);
    let right = parse(right);
    for index in 0..left.core.len().max(right.core.len()) {
        let ordering = left
            .core
            .get(index)
            .copied()
            .unwrap_or(0)
            .cmp(&right.core.get(index).copied().unwrap_or(0));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    match (left.pre.is_empty(), right.pre.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    for (left, right) in left.pre.iter().zip(&right.pre) {
        let ordering = match (left.parse::<i64>(), right.parse::<i64>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            _ => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.pre.len().cmp(&right.pre.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_existing_release_and_prerelease_forms() {
        for (left, right, ordering) in [
            ("v0.3.0", "0.3.0+4", Ordering::Equal),
            ("0.3.10", "0.3.9", Ordering::Greater),
            ("0.9.0", "0.10.0", Ordering::Less),
            ("0.4", "0.4.0", Ordering::Equal),
            ("0.4.0", "0.4.0-beta.1", Ordering::Greater),
            ("0.4.0-beta.2", "0.4.0-beta.1", Ordering::Greater),
            ("0.4.0-beta", "0.4.0-beta.1", Ordering::Less),
            ("0.4.0-1", "0.4.0-alpha", Ordering::Less),
            ("latest", "0.3.0", Ordering::Less),
        ] {
            assert_eq!(compare(left, right), ordering);
        }
    }
}
