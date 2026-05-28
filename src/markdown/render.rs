use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::inline::{self, Inline};
use crate::theme;

#[allow(dead_code)]
pub struct LinkSpan {
    pub target: String,
    pub heading: Option<String>,
    pub embed: bool,
    pub resolved: bool,
    pub row: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub span_idx: usize,
}

pub struct RenderedNote {
    pub lines: Vec<Line<'static>>,
    #[allow(dead_code)]
    pub links: Vec<LinkSpan>,
}

struct Seg {
    text: String,
    style: Style,
    link: Option<usize>,
}

struct LinkMeta {
    target: String,
    heading: Option<String>,
    embed: bool,
    resolved: bool,
}

struct Builder<F: Fn(&str) -> bool> {
    width: usize,
    logical: Vec<Vec<Seg>>,
    cur: Vec<Seg>,
    style: Vec<Style>,
    list: Vec<Option<u64>>,
    bq: usize,
    code: bool,
    pending: String,
    links: Vec<LinkMeta>,
    resolves: F,
}

impl<F: Fn(&str) -> bool> Builder<F> {
    fn new(width: usize, resolves: F) -> Self {
        Self {
            width: width.max(1),
            logical: Vec::new(),
            cur: Vec::new(),
            style: Vec::new(),
            list: Vec::new(),
            bq: 0,
            code: false,
            pending: String::new(),
            links: Vec::new(),
            resolves,
        }
    }

    fn cur_style(&self) -> Style {
        self.style
            .iter()
            .fold(theme::text(), |acc, s| acc.patch(*s))
    }

    fn flush(&mut self) {
        if self.cur.is_empty() {
            return;
        }
        let mut line = Vec::new();
        if self.bq > 0 {
            line.push(Seg {
                text: "│ ".repeat(self.bq),
                style: theme::muted(),
                link: None,
            });
        }
        line.append(&mut self.cur);
        self.logical.push(line);
    }

    fn block_gap(&mut self) {
        self.flush();
        if self.logical.last().is_some_and(|l| !l.is_empty()) {
            self.logical.push(Vec::new());
        }
    }

    fn event(&mut self, ev: Event) {
        if let Event::Text(t) = &ev {
            if self.code {
                self.push_code_text(t);
            } else {
                self.pending.push_str(t);
            }
            return;
        }
        if matches!(ev, Event::SoftBreak) {
            if !self.code {
                self.pending.push(' ');
            }
            return;
        }
        self.flush_pending();
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Code(t) => self.cur.push(Seg {
                text: format!("`{t}`"),
                style: theme::code(),
                link: None,
            }),
            Event::HardBreak => self.flush(),
            Event::Rule => {
                self.flush();
                self.logical.push(vec![Seg {
                    text: "─".repeat(self.width),
                    style: theme::faint(),
                    link: None,
                }]);
            }
            Event::TaskListMarker(done) => self.cur.push(Seg {
                text: if done { "[x] " } else { "[ ] " }.to_string(),
                style: theme::muted(),
                link: None,
            }),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                if self.list.is_empty() {
                    self.block_gap();
                }
            }
            Tag::Heading { level, .. } => {
                self.block_gap();
                self.style.push(heading_style(level));
            }
            Tag::BlockQuote(_) => {
                self.bq += 1;
                if self.list.is_empty() {
                    self.block_gap();
                }
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = true;
            }
            Tag::List(start) => self.list.push(start),
            Tag::Item => {
                self.flush();
                self.emit_item_prefix();
            }
            Tag::Emphasis => self.style.push(Style::default().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self.style.push(Style::default().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self
                .style
                .push(Style::default().add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { .. } => self.style.push(theme::link()),
            Tag::Image { .. } => self.style.push(theme::muted()),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                self.flush();
                self.style.pop();
            }
            TagEnd::BlockQuote(_) => self.bq = self.bq.saturating_sub(1),
            TagEnd::CodeBlock => {
                self.flush();
                self.code = false;
            }
            TagEnd::List(_) => {
                self.list.pop();
            }
            TagEnd::Item => self.flush(),
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Image => {
                self.style.pop();
            }
            _ => {}
        }
    }

    fn emit_item_prefix(&mut self) {
        let depth = self.list.len().saturating_sub(1);
        let indent = "  ".repeat(depth);
        let marker = match self.list.last_mut() {
            Some(Some(n)) => {
                let m = format!("{n}. ");
                *n += 1;
                m
            }
            _ => "• ".to_string(),
        };
        self.cur.push(Seg {
            text: format!("{indent}{marker}"),
            style: theme::muted(),
            link: None,
        });
    }

    fn push_code_text(&mut self, text: &str) {
        let mut first = true;
        for part in text.split('\n') {
            if !first {
                self.flush();
            }
            first = false;
            if !part.is_empty() {
                self.cur.push(Seg {
                    text: part.to_string(),
                    style: theme::code(),
                    link: None,
                });
            }
        }
    }

    fn flush_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.pending);
        let style = self.cur_style();
        for piece in inline::scan(&text) {
            match piece {
                Inline::Text(r) => self.cur.push(Seg {
                    text: text[r].to_string(),
                    style,
                    link: None,
                }),
                Inline::Tag(r) => self.cur.push(Seg {
                    text: text[r].to_string(),
                    style: theme::tag(),
                    link: None,
                }),
                Inline::Wikilink(data) => {
                    let display = data.alias.clone().unwrap_or_else(|| data.target.clone());
                    let resolved = (self.resolves)(&data.target);
                    let idx = self.links.len();
                    self.links.push(LinkMeta {
                        target: data.target.clone(),
                        heading: data.heading.clone(),
                        embed: data.embed,
                        resolved,
                    });
                    let style = if resolved {
                        theme::link()
                    } else {
                        theme::phantom()
                    };
                    let shown = if data.embed {
                        format!("!{display}")
                    } else {
                        display
                    };
                    self.cur.push(Seg {
                        text: shown,
                        style,
                        link: Some(idx),
                    });
                }
            }
        }
    }

    fn finish(mut self) -> RenderedNote {
        self.flush_pending();
        self.flush();
        wrap(self.logical, self.links, self.width)
    }
}

fn heading_style(_level: HeadingLevel) -> Style {
    Style::default().fg(theme::EMPH).add_modifier(Modifier::BOLD)
}

struct Unit {
    text: String,
    style: Style,
    link: Option<usize>,
    is_space: bool,
}

impl Unit {
    fn width(&self) -> usize {
        UnicodeWidthStr::width(self.text.as_str())
    }
}

fn split_words(text: &str, style: Style, out: &mut Vec<Unit>) {
    let mut buf = String::new();
    let mut is_space: Option<bool> = None;
    for ch in text.chars() {
        let sp = ch.is_whitespace();
        match is_space {
            Some(b) if b == sp => buf.push(ch),
            Some(prev) => {
                out.push(Unit {
                    text: std::mem::take(&mut buf),
                    style,
                    link: None,
                    is_space: prev,
                });
                buf.push(ch);
                is_space = Some(sp);
            }
            None => {
                buf.push(ch);
                is_space = Some(sp);
            }
        }
    }
    if let Some(prev) = is_space
        && !buf.is_empty()
    {
        out.push(Unit {
            text: buf,
            style,
            link: None,
            is_space: prev,
        });
    }
}

fn wrap(logical: Vec<Vec<Seg>>, links: Vec<LinkMeta>, width: usize) -> RenderedNote {
    let width = width.max(1);
    let mut out_lines: Vec<Line<'static>> = Vec::new();
    let mut link_spans: Vec<LinkSpan> = Vec::new();

    for segs in logical {
        let mut units: Vec<Unit> = Vec::new();
        for seg in segs {
            if seg.link.is_some() {
                units.push(Unit {
                    text: seg.text,
                    style: seg.style,
                    link: seg.link,
                    is_space: false,
                });
            } else {
                split_words(&seg.text, seg.style, &mut units);
            }
        }

        let mut row_spans: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;
        let base_row = out_lines.len();
        let mut row = 0usize;

        for unit in units {
            let w = unit.width();
            if col + w > width && col > 0 {
                out_lines.push(Line::from(std::mem::take(&mut row_spans)));
                row += 1;
                col = 0;
                if unit.is_space {
                    continue;
                }
            }
            if let Some(li) = unit.link {
                link_spans.push(LinkSpan {
                    target: links[li].target.clone(),
                    heading: links[li].heading.clone(),
                    embed: links[li].embed,
                    resolved: links[li].resolved,
                    row: base_row + row,
                    col_start: col,
                    col_end: col + w,
                    span_idx: row_spans.len(),
                });
            }
            row_spans.push(Span::styled(unit.text, unit.style));
            col += w;
        }
        out_lines.push(Line::from(std::mem::take(&mut row_spans)));
    }

    RenderedNote {
        lines: out_lines,
        links: link_spans,
    }
}

pub fn render<F: Fn(&str) -> bool>(src: &str, width: u16, resolves: F) -> RenderedNote {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let mut builder = Builder::new(width as usize, resolves);
    for ev in Parser::new_ext(src, opts) {
        builder.event(ev);
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_links_and_respects_width() {
        let src = "# Title\n\nThis is [[Foo]] and #bar text that wraps over several short lines.";
        let r = render(src, 20, |t| t == "Foo");
        assert!(r.links.iter().any(|l| l.target == "Foo" && l.resolved));
        for line in &r.lines {
            let w: usize = line
                .spans
                .iter()
                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                .sum();
            assert!(w <= 20, "line width {w} exceeds 20");
        }
    }

    #[test]
    fn phantom_links_are_marked_unresolved() {
        let r = render("[[Nope]]", 40, |_| false);
        assert_eq!(r.links.len(), 1);
        assert!(!r.links[0].resolved);
        assert_eq!(r.links[0].target, "Nope");
    }

    #[test]
    fn wikilink_split_across_text_events_is_reassembled() {
        let r = render("see [[My Note|shown]] here", 80, |t| t == "My Note");
        assert_eq!(r.links.len(), 1);
        assert!(r.links[0].resolved);
        assert_eq!(r.links[0].target, "My Note");
    }
}
