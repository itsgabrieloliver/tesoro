mod conceal;
mod inline;
mod live;
mod render;

pub use conceal::conceal_line;
pub use live::{RowKind, editor_plan};
pub use render::{RenderedNote, render};

pub fn wikilink_targets(text: &str) -> Vec<String> {
    inline::scan(text)
        .into_iter()
        .filter_map(|p| match p {
            inline::Inline::Wikilink(d) => Some(d.target),
            _ => None,
        })
        .collect()
}

pub fn wikilink_positions(line: &str) -> Vec<(usize, usize, String)> {
    inline::wikilink_matches(line)
        .into_iter()
        .map(|(bs, be, target)| (line[..bs].chars().count(), line[..be].chars().count(), target))
        .collect()
}

pub fn tag_names(text: &str) -> Vec<String> {
    inline::scan(text)
        .into_iter()
        .filter_map(|p| match p {
            inline::Inline::Tag(r) => Some(text[r].trim_start_matches('#').to_string()),
            _ => None,
        })
        .collect()
}
