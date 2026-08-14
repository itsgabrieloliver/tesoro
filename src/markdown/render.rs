use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
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

struct LogicalLine {
    segs: Vec<Seg>,
    raw: bool,
}

struct LinkMeta {
    target: String,
    heading: Option<String>,
    embed: bool,
    resolved: bool,
}

struct TableBuf {
    aligns: Vec<Alignment>,
    header: Vec<String>,
    body: Vec<Vec<String>>,
    cur_row: Vec<String>,
    cur_cell: String,
    in_head: bool,
}

struct Builder<F: Fn(&str) -> bool> {
    width: usize,
    logical: Vec<LogicalLine>,
    cur: Vec<Seg>,
    style: Vec<Style>,
    list: Vec<Option<u64>>,
    bq: usize,
    code: bool,
    callout: Option<ratatui::style::Color>,
    pending: String,
    table: Option<TableBuf>,
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
            callout: None,
            pending: String::new(),
            table: None,
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
            let style = match self.callout {
                Some(c) => Style::default().fg(c),
                None => theme::muted(),
            };
            line.push(Seg {
                text: "│ ".repeat(self.bq),
                style,
                link: None,
            });
        }
        line.append(&mut self.cur);
        self.logical.push(LogicalLine {
            segs: line,
            raw: false,
        });
    }

    fn block_gap(&mut self) {
        self.flush();
        if self.logical.last().is_some_and(|l| !l.segs.is_empty()) {
            self.logical.push(LogicalLine {
                segs: Vec::new(),
                raw: false,
            });
        }
    }

    fn event(&mut self, ev: Event) {
        if self.table.is_some() {
            match &ev {
                Event::Text(t) | Event::Code(t) => {
                    let cell = &mut self.table.as_mut().unwrap().cur_cell;
                    if matches!(ev, Event::Code(_)) {
                        cell.push('`');
                        cell.push_str(t);
                        cell.push('`');
                    } else {
                        cell.push_str(t);
                    }
                    return;
                }
                Event::SoftBreak | Event::HardBreak => {
                    self.table.as_mut().unwrap().cur_cell.push(' ');
                    return;
                }
                _ => {}
            }
        }
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
                if self.bq > 0
                    && self.callout.is_none()
                    && self.pending.trim_start().starts_with("[!")
                {
                    self.flush_pending();
                } else {
                    self.pending.push(' ');
                }
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
                self.logical.push(LogicalLine {
                    segs: vec![Seg {
                        text: "─".repeat(self.width),
                        style: theme::faint(),
                        link: None,
                    }],
                    raw: false,
                });
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
            Tag::Table(aligns) => {
                self.block_gap();
                self.table = Some(TableBuf {
                    aligns,
                    header: Vec::new(),
                    body: Vec::new(),
                    cur_row: Vec::new(),
                    cur_cell: String::new(),
                    in_head: false,
                });
            }
            Tag::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.in_head = true;
                    t.cur_row.clear();
                }
            }
            Tag::TableRow => {
                if let Some(t) = self.table.as_mut() {
                    t.cur_row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(t) = self.table.as_mut() {
                    t.cur_cell.clear();
                }
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
            TagEnd::Heading(level) => {
                let w: usize = self
                    .cur
                    .iter()
                    .map(|s| UnicodeWidthStr::width(s.text.as_str()))
                    .sum();
                self.flush();
                self.style.pop();
                if w > 0
                    && let Some((ch, style)) = heading_underline(level)
                {
                    self.logical.push(LogicalLine {
                        segs: vec![Seg {
                            text: ch.to_string().repeat(w.min(self.width)),
                            style,
                            link: None,
                        }],
                        raw: false,
                    });
                }
            }
            TagEnd::BlockQuote(_) => {
                self.bq = self.bq.saturating_sub(1);
                if self.bq == 0 {
                    self.callout = None;
                }
            }
            TagEnd::CodeBlock => {
                self.flush();
                self.code = false;
            }
            TagEnd::List(_) => {
                self.list.pop();
            }
            TagEnd::Item => self.flush(),
            TagEnd::TableCell => {
                if let Some(t) = self.table.as_mut() {
                    let cell = std::mem::take(&mut t.cur_cell);
                    t.cur_row.push(cell.trim().to_string());
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.header = std::mem::take(&mut t.cur_row);
                    t.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(t) = self.table.as_mut()
                    && !t.in_head
                {
                    let row = std::mem::take(&mut t.cur_row);
                    t.body.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.emit_table(t);
                }
            }
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

    fn emit_table(&mut self, t: TableBuf) {
        let ncols = t
            .header
            .len()
            .max(t.body.iter().map(|r| r.len()).max().unwrap_or(0));
        if ncols == 0 {
            return;
        }

        let mut widths = vec![1usize; ncols];
        for (i, w) in widths.iter_mut().enumerate() {
            let mut max = t
                .header
                .get(i)
                .map(|s| UnicodeWidthStr::width(s.as_str()))
                .unwrap_or(0);
            for row in &t.body {
                if let Some(c) = row.get(i) {
                    max = max.max(UnicodeWidthStr::width(c.as_str()));
                }
            }
            *w = max.max(1);
        }

        let overhead = 3 * ncols + 1;
        let avail = self.width.saturating_sub(overhead).max(ncols);
        let mut total: usize = widths.iter().sum();
        while total > avail {
            let idx = widths
                .iter()
                .enumerate()
                .max_by_key(|(_, w)| **w)
                .map(|(i, _)| i)
                .unwrap();
            if widths[idx] <= 1 {
                break;
            }
            widths[idx] -= 1;
            total -= 1;
        }

        self.logical.push(table_rule("┌", "┬", "┐", &widths));
        self.logical
            .push(table_row(&t.header, &widths, &t.aligns, true));
        self.logical.push(table_rule("├", "┼", "┤", &widths));
        for row in &t.body {
            self.logical
                .push(table_row(row, &widths, &t.aligns, false));
        }
        self.logical.push(table_rule("└", "┴", "┘", &widths));
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
        if self.bq > 0
            && self.callout.is_none()
            && let Some(rest) = text.trim_start().strip_prefix("[!")
            && let Some(close) = rest.find(']')
        {
            let kind = rest[..close].to_ascii_lowercase();
            let (icon, color) = theme::callout(&kind);
            self.callout = Some(color);
            let title_rest = rest[close + 1..].trim().to_string();
            let title = if title_rest.is_empty() {
                let mut t = kind;
                if let Some(f) = t.get_mut(0..1) {
                    f.make_ascii_uppercase();
                }
                t
            } else {
                title_rest
            };
            self.cur.push(Seg {
                text: format!("{icon} {title}"),
                style: Style::default().fg(color).add_modifier(Modifier::BOLD),
                link: None,
            });
            self.flush();
            return;
        }
        let style = self.cur_style();
        for piece in inline::scan(&text) {
            match piece {
                Inline::Text(r) => self.cur.push(Seg {
                    text: text[r].to_string(),
                    style,
                    link: None,
                }),
                Inline::Tag(r) => {
                    let name = text[r.clone()].trim_start_matches('#').to_string();
                    self.cur.push(Seg {
                        text: text[r].to_string(),
                        style: theme::tag_for(&name),
                        link: None,
                    })
                }
                Inline::Highlight(s) => self.cur.push(Seg {
                    text: s,
                    style: style.patch(theme::highlight()),
                    link: None,
                }),
                Inline::Color { spec, text: t } => {
                    let st = match theme::named_color(&spec) {
                        Some(c) => style.fg(c),
                        None => style,
                    };
                    self.cur.push(Seg {
                        text: t,
                        style: st,
                        link: None,
                    })
                }
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

pub(crate) fn heading_style(level: HeadingLevel) -> Style {
    use HeadingLevel::*;
    let base = Style::default().add_modifier(Modifier::BOLD);
    match level {
        H1 | H2 => base.fg(theme::EMPH),
        H3 => base.fg(theme::ACCENT),
        H4 => base.fg(theme::TEXT),
        H5 => base.fg(theme::MUTED),
        H6 => base.fg(theme::MUTED).add_modifier(Modifier::ITALIC),
    }
}

pub(crate) fn heading_underline(level: HeadingLevel) -> Option<(char, Style)> {
    match level {
        HeadingLevel::H1 => Some(('═', Style::default().fg(theme::EMPH))),
        HeadingLevel::H2 => Some(('─', Style::default().fg(theme::MUTED))),
        _ => None,
    }
}

fn table_rule(left: &str, mid: &str, right: &str, widths: &[usize]) -> LogicalLine {
    let mut text = String::from(left);
    for (i, w) in widths.iter().enumerate() {
        if i > 0 {
            text.push_str(mid);
        }
        text.push_str(&"─".repeat(w + 2));
    }
    text.push_str(right);
    LogicalLine {
        segs: vec![Seg {
            text,
            style: theme::faint(),
            link: None,
        }],
        raw: true,
    }
}

fn table_row(cells: &[String], widths: &[usize], aligns: &[Alignment], header: bool) -> LogicalLine {
    let border = theme::faint();
    let cell_style = if header { theme::brand() } else { theme::text() };
    let mut segs = vec![Seg {
        text: "│".to_string(),
        style: border,
        link: None,
    }];
    for (i, w) in widths.iter().enumerate() {
        let raw = cells.get(i).map(String::as_str).unwrap_or("");
        let align = aligns.get(i).copied().unwrap_or(Alignment::None);
        let cell = pad(&truncate_to(raw, *w), *w, align);
        segs.push(Seg {
            text: format!(" {cell} "),
            style: cell_style,
            link: None,
        });
        segs.push(Seg {
            text: "│".to_string(),
            style: border,
            link: None,
        });
    }
    LogicalLine { segs, raw: true }
}

fn truncate_to(s: &str, max: usize) -> String {
    if UnicodeWidthStr::width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthStr::width(ch.to_string().as_str());
        if w + cw > max.saturating_sub(1) {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}

fn pad(s: &str, width: usize, align: Alignment) -> String {
    let w = UnicodeWidthStr::width(s);
    if w >= width {
        return s.to_string();
    }
    let extra = width - w;
    match align {
        Alignment::Right => format!("{}{s}", " ".repeat(extra)),
        Alignment::Center => {
            let l = extra / 2;
            let r = extra - l;
            format!("{}{s}{}", " ".repeat(l), " ".repeat(r))
        }
        _ => format!("{s}{}", " ".repeat(extra)),
    }
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

fn wrap(logical: Vec<LogicalLine>, links: Vec<LinkMeta>, width: usize) -> RenderedNote {
    let width = width.max(1);
    let mut out_lines: Vec<Line<'static>> = Vec::new();
    let mut link_spans: Vec<LinkSpan> = Vec::new();

    for line in logical {
        if line.raw {
            let spans: Vec<Span<'static>> = line
                .segs
                .into_iter()
                .map(|s| Span::styled(s.text, s.style))
                .collect();
            out_lines.push(Line::from(spans));
            continue;
        }

        let mut units: Vec<Unit> = Vec::new();
        for seg in line.segs {
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
    opts.insert(Options::ENABLE_TABLES);
    let mut builder = Builder::new(width as usize, resolves);
    for ev in Parser::new_ext(src, opts) {
        builder.event(ev);
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

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

    #[test]
    fn headings_get_level_specific_underlines() {
        let r = render("# One\n\n## Two\n\n### Three", 80, |_| false);
        let texts: Vec<String> = r.lines.iter().map(line_text).collect();
        assert!(texts.iter().any(|t| t == "One"));
        assert!(
            texts.iter().any(|t| t.starts_with('═') && t.chars().count() == 3),
            "H1 should get a ═ underline matching its width"
        );
        assert!(
            texts.iter().any(|t| t.starts_with('─') && t.chars().count() == 3),
            "H2 should get a ─ underline matching its width"
        );
        assert!(
            !texts.iter().any(|t| *t == "═".repeat(80) || *t == "─".repeat(80)),
            "H3 should not be underlined"
        );
    }

    #[test]
    fn heading_styles_differ_by_level() {
        let h1 = heading_style(HeadingLevel::H1);
        let h3 = heading_style(HeadingLevel::H3);
        let h6 = heading_style(HeadingLevel::H6);
        assert_ne!(h1.fg, h3.fg);
        assert!(h6.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn highlights_and_color_spans_render_styled() {
        let r = render("==hot== and {red:fire}", 80, |_| false);
        let joined: String = r.lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("hot") && !joined.contains("=="));
        assert!(joined.contains("fire") && !joined.contains("{red:"));
        assert!(
            r.lines
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| s.style.bg == Some(theme::HL_BG)),
            "highlight background applied"
        );
        assert!(
            r.lines
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| s.style.fg == Some(theme::DANGER)),
            "red color span applied"
        );
    }

    #[test]
    fn tags_get_distinct_palette_colors() {
        let r = render("#a #b", 80, |_| false);
        let fgs: Vec<_> = r
            .lines
            .iter()
            .flat_map(|l| &l.spans)
            .filter(|s| s.content.starts_with('#'))
            .map(|s| s.style.fg)
            .collect();
        assert_eq!(fgs.len(), 2);
        assert_ne!(fgs[0], fgs[1]);
    }

    #[test]
    fn callout_renders_icon_title_and_colored_border() {
        let r = render("> [!tip] Hydrate\n> drink water\n", 80, |_| false);
        let joined: String = r.lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("✓ Hydrate"));
        assert!(joined.contains("drink water"));
        assert!(!joined.contains("[!tip]"));
        assert!(
            r.lines
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| s.content.contains('│') && s.style.fg == Some(theme::SUCCESS)),
            "callout border takes the callout color"
        );
    }

    #[test]
    fn renders_a_table_with_box_borders() {
        let src = "| col 1 | col 2 |\n| ----- | ----- |\n| a | bb |\n";
        let r = render(src, 80, |_| false);
        let texts: Vec<String> = r.lines.iter().map(line_text).collect();
        let joined = texts.join("\n");
        assert!(joined.contains('┌') && joined.contains('┐'), "top border");
        assert!(joined.contains('├') && joined.contains('┤'), "header sep");
        assert!(joined.contains('└') && joined.contains('┘'), "bottom border");
        assert!(texts.iter().any(|t| t.contains("col 1") && t.contains("col 2")));
        assert!(texts.iter().any(|t| t.contains('a') && t.contains("bb")));
    }

    #[test]
    fn table_rows_are_not_word_wrapped() {
        let src = "| name | value |\n| ---- | ----- |\n| x | y |\n";
        let r = render(src, 80, |_| false);
        let border_rows = r
            .lines
            .iter()
            .map(line_text)
            .filter(|t| t.starts_with('│'))
            .count();
        assert_eq!(border_rows, 2, "header + one body row, each on a single line");
    }

    #[test]
    fn wide_table_is_truncated_to_width() {
        let src = "| aaaaaaaaaa | bbbbbbbbbb |\n| --- | --- |\n| cccccccccc | dddddddddd |\n";
        let r = render(src, 16, |_| false);
        for line in &r.lines {
            let w: usize = line
                .spans
                .iter()
                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                .sum();
            assert!(w <= 16, "table line width {w} exceeds 16");
        }
    }
}
