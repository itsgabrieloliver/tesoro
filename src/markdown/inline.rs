use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

static WIKILINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?P<embed>!)?\[\[(?P<target>[^\[\]|#^]+?)(?:#\^(?P<block>[^\[\]|]+?)|#(?P<heading>[^\[\]|]+?))?(?:\|(?P<alias>[^\[\]]+?))?\]\]",
    )
    .unwrap()
});

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#(?P<tag>[A-Za-z][\w/\-]*)").unwrap());

pub struct WikilinkData {
    pub target: String,
    pub heading: Option<String>,
    #[allow(dead_code)]
    pub block: Option<String>,
    pub alias: Option<String>,
    pub embed: bool,
}

pub enum Inline {
    Text(Range<usize>),
    Wikilink(WikilinkData),
    Tag(Range<usize>),
}

enum Mark {
    Wiki(WikilinkData),
    Tag,
}

pub fn scan(text: &str) -> Vec<Inline> {
    let mut marks: Vec<(usize, usize, Mark)> = Vec::new();

    for caps in WIKILINK.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        let data = WikilinkData {
            target: caps.name("target").unwrap().as_str().trim().to_string(),
            heading: caps.name("heading").map(|m| m.as_str().trim().to_string()),
            block: caps.name("block").map(|m| m.as_str().trim().to_string()),
            alias: caps.name("alias").map(|m| m.as_str().trim().to_string()),
            embed: caps.name("embed").is_some(),
        };
        marks.push((whole.start(), whole.end(), Mark::Wiki(data)));
    }

    for caps in TAG.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        let preceded_ok = whole.start() == 0
            || text[..whole.start()]
                .chars()
                .next_back()
                .map(|ch| ch.is_whitespace() || "([{<\"'".contains(ch))
                .unwrap_or(true);
        if preceded_ok {
            marks.push((whole.start(), whole.end(), Mark::Tag));
        }
    }

    marks.sort_by_key(|m| m.0);

    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut last_end = 0usize;
    for (start, end, mark) in marks {
        if start < last_end {
            continue;
        }
        if start > pos {
            out.push(Inline::Text(pos..start));
        }
        match mark {
            Mark::Wiki(data) => out.push(Inline::Wikilink(data)),
            Mark::Tag => out.push(Inline::Tag(start..end)),
        }
        pos = end;
        last_end = end;
    }
    if pos < text.len() {
        out.push(Inline::Text(pos..text.len()));
    }
    out
}

pub fn wikilink_matches(text: &str) -> Vec<(usize, usize, String)> {
    WIKILINK
        .captures_iter(text)
        .map(|c| {
            let whole = c.get(0).unwrap();
            (
                whole.start(),
                whole.end(),
                c.name("target").unwrap().as_str().trim().to_string(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_wikilink_with_alias_and_heading() {
        let pieces = scan("see [[Target#Sec|Shown]] now");
        let data = pieces
            .iter()
            .find_map(|p| match p {
                Inline::Wikilink(d) => Some(d),
                _ => None,
            })
            .unwrap();
        assert_eq!(data.target, "Target");
        assert_eq!(data.heading.as_deref(), Some("Sec"));
        assert_eq!(data.alias.as_deref(), Some("Shown"));
    }

    #[test]
    fn tag_requires_a_leading_boundary() {
        let text = "a #real but not C#sharp";
        let pieces = scan(text);
        let tags: Vec<&str> = pieces
            .iter()
            .filter_map(|p| match p {
                Inline::Tag(r) => Some(&text[r.clone()]),
                _ => None,
            })
            .collect();
        assert!(tags.contains(&"#real"));
        assert!(!tags.iter().any(|t| t.contains("sharp")));
    }
}
