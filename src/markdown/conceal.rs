use std::sync::LazyLock;

use pulldown_cmark::HeadingLevel;
use ratatui::style::{Modifier, Style};
use regex::Regex;

use super::inline;
use super::render::heading_style;
use crate::theme;

static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*[^*]+\*\*").unwrap());
static ITALIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*[^*]+\*").unwrap());
static STRIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"~~[^~]+~~").unwrap());
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`]+`").unwrap());

pub fn conceal_line(line: &str) -> Vec<(String, Style)> {
    if let Some((level, rest)) = heading_split(line) {
        return spans(rest, heading_style(level));
    }
    spans(line, theme::text())
}

fn heading_split(s: &str) -> Option<(HeadingLevel, &str)> {
    let n = s.bytes().take_while(|b| *b == b'#').count();
    if n == 0 || n > 6 {
        return None;
    }
    let rest = s[n..].strip_prefix(' ')?;
    let level = match n {
        1 => HeadingLevel::H1,
        2 => HeadingLevel::H2,
        3 => HeadingLevel::H3,
        4 => HeadingLevel::H4,
        5 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    };
    Some((level, rest))
}

fn spans(text: &str, base: Style) -> Vec<(String, Style)> {
    let mut marks: Vec<(usize, usize, String, Style)> = Vec::new();
    for m in CODE.find_iter(text) {
        marks.push((
            m.start(),
            m.end(),
            inner_of(m.as_str(), 1),
            base.patch(theme::code()),
        ));
    }
    for (s, e, _target) in inline::wikilink_matches(text) {
        marks.push((s, e, link_display(&text[s..e]), base.patch(theme::link())));
    }
    for (s, e, spec, inner) in inline::color_matches(text) {
        let st = theme::named_color(&spec)
            .map(|c| base.fg(c))
            .unwrap_or(base);
        marks.push((s, e, inner, st));
    }
    for (s, e) in inline::highlight_matches(text) {
        marks.push((
            s,
            e,
            inner_of(&text[s..e], 2),
            base.patch(theme::highlight()),
        ));
    }
    for m in BOLD.find_iter(text) {
        marks.push((
            m.start(),
            m.end(),
            inner_of(m.as_str(), 2),
            base.add_modifier(Modifier::BOLD),
        ));
    }
    for m in STRIKE.find_iter(text) {
        marks.push((
            m.start(),
            m.end(),
            inner_of(m.as_str(), 2),
            base.add_modifier(Modifier::CROSSED_OUT),
        ));
    }
    let mut masked = text.as_bytes().to_vec();
    for m in BOLD.find_iter(text) {
        masked[m.start()..m.end()].fill(0);
    }
    let masked = String::from_utf8(masked).unwrap_or_default();
    for m in ITALIC.find_iter(&masked) {
        marks.push((
            m.start(),
            m.end(),
            inner_of(&text[m.start()..m.end()], 1),
            base.add_modifier(Modifier::ITALIC),
        ));
    }
    for (s, e) in inline::tag_matches(text) {
        let name = text[s..e].trim_start_matches('#').to_string();
        marks.push((s, e, text[s..e].to_string(), base.patch(theme::tag_for(&name))));
    }
    marks.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut out = Vec::new();
    let mut pos = 0usize;
    for (s, e, txt, st) in marks {
        if s < pos {
            continue;
        }
        if s > pos {
            out.push((text[pos..s].to_string(), base));
        }
        out.push((txt, st));
        pos = e;
    }
    if pos < text.len() {
        out.push((text[pos..].to_string(), base));
    }
    if out.is_empty() {
        out.push((String::new(), base));
    }
    out
}

fn inner_of(s: &str, n: usize) -> String {
    s[n..s.len() - n].to_string()
}

fn link_display(raw: &str) -> String {
    if raw.len() < 4 {
        return raw.to_string();
    }
    let inner = raw[2..raw.len() - 2].trim();
    if let Some(idx) = inner.find('|') {
        return inner[idx + 1..].trim().to_string();
    }
    let t = inner.split('#').next().unwrap_or(inner);
    t.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(line: &str) -> String {
        conceal_line(line)
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    #[test]
    fn heading_marker_is_stripped_and_styled() {
        let segs = conceal_line("## Section Title");
        let text: String = segs.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(text, "Section Title");
        assert!(segs[0].1.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn inline_markup_is_concealed() {
        assert_eq!(
            joined("a **b** `c` [[D|shown]] ==e== {red:f} *g* ~~h~~"),
            "a b c shown e f g h"
        );
    }

    #[test]
    fn tag_without_space_is_not_a_heading() {
        assert_eq!(joined("#tag here"), "#tag here");
    }

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(joined("nothing special"), "nothing special");
    }
}
