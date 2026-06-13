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

static HIGHLIGHT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"==(?P<hl>[^=]+)==").unwrap());

static COLOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{(?P<col>[A-Za-z]+|#[0-9A-Fa-f]{6}):(?P<txt>[^{}]+)\}").unwrap()
});

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
    Highlight(String),
    Color { spec: String, text: String },
}

enum Mark {
    Wiki(WikilinkData),
    Tag,
    Highlight(String),
    Color { spec: String, text: String },
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

    for caps in COLOR.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        marks.push((
            whole.start(),
            whole.end(),
            Mark::Color {
                spec: caps.name("col").unwrap().as_str().to_string(),
                text: caps.name("txt").unwrap().as_str().to_string(),
            },
        ));
    }

    for caps in HIGHLIGHT.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        marks.push((
            whole.start(),
            whole.end(),
            Mark::Highlight(caps.name("hl").unwrap().as_str().to_string()),
        ));
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
            Mark::Highlight(s) => out.push(Inline::Highlight(s)),
            Mark::Color { spec, text } => out.push(Inline::Color { spec, text }),
        }
        pos = end;
        last_end = end;
    }
    if pos < text.len() {
        out.push(Inline::Text(pos..text.len()));
    }
    out
}

pub fn tag_matches(text: &str) -> Vec<(usize, usize)> {
    TAG.captures_iter(text)
        .filter_map(|caps| {
            let whole = caps.get(0).unwrap();
            let preceded_ok = whole.start() == 0
                || text[..whole.start()]
                    .chars()
                    .next_back()
                    .map(|ch| ch.is_whitespace() || "([{<\"'".contains(ch))
                    .unwrap_or(true);
            preceded_ok.then(|| (whole.start(), whole.end()))
        })
        .collect()
}

pub fn highlight_matches(text: &str) -> Vec<(usize, usize)> {
    HIGHLIGHT
        .find_iter(text)
        .map(|m| (m.start(), m.end()))
        .collect()
}

pub fn color_matches(text: &str) -> Vec<(usize, usize, String, String)> {
    COLOR
        .captures_iter(text)
        .map(|c| {
            let w = c.get(0).unwrap();
            (
                w.start(),
                w.end(),
                c.name("col").unwrap().as_str().to_string(),
                c.name("txt").unwrap().as_str().to_string(),
            )
        })
        .collect()
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
    fn finds_highlights_and_color_spans() {
        let pieces = scan("x ==hi== {red:stop} y");
        assert!(
            pieces
                .iter()
                .any(|p| matches!(p, Inline::Highlight(s) if s == "hi"))
        );
        assert!(pieces.iter().any(
            |p| matches!(p, Inline::Color { spec, text } if spec == "red" && text == "stop")
        ));
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
