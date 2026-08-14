use pulldown_cmark::HeadingLevel;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

use super::conceal_line;
use super::render::{heading_underline, render};
use crate::theme;

pub enum RowKind {
    Detailed { base: Style },
    Spans(Vec<(String, Style)>),
}

pub struct DisplayRow {
    pub src: Option<usize>,
    pub kind: RowKind,
}

pub fn editor_plan(
    lines: &[String],
    cursor_line: usize,
    width: usize,
    conceal: bool,
) -> Vec<DisplayRow> {
    if !conceal {
        return lines
            .iter()
            .enumerate()
            .map(|(i, _)| DisplayRow {
                src: Some(i),
                kind: RowKind::Detailed {
                    base: theme::text(),
                },
            })
            .collect();
    }

    let mut rows = Vec::with_capacity(lines.len() + 4);
    let mut i = 0usize;
    let mut in_code = false;
    while i < lines.len() {
        let line = &lines[i];
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            in_code = !in_code;
            rows.push(line_row(lines, i, cursor_line));
            i += 1;
            continue;
        }
        if in_code {
            rows.push(line_row(lines, i, cursor_line));
            i += 1;
            continue;
        }

        if is_table_start(lines, i) {
            let end = table_end(lines, i);
            let cursor_in = cursor_line >= i && cursor_line < end;
            if cursor_in {
                for j in i..end {
                    if j == cursor_line {
                        rows.push(DisplayRow {
                            src: Some(j),
                            kind: RowKind::Detailed {
                                base: theme::text(),
                            },
                        });
                    } else {
                        rows.push(DisplayRow {
                            src: Some(j),
                            kind: RowKind::Spans(vec![(lines[j].clone(), theme::text())]),
                        });
                    }
                }
            } else {
                for spans in table_grid(&lines[i..end], width) {
                    rows.push(DisplayRow {
                        src: Some(i),
                        kind: RowKind::Spans(spans),
                    });
                }
            }
            i = end;
            continue;
        }

        if trimmed.starts_with('>') {
            let end = blockquote_end(lines, i);
            let callout = callout_color(&lines[i]);
            for j in i..end {
                if j == cursor_line {
                    let base = match callout {
                        Some(c) => Style::default().fg(c),
                        None => theme::muted(),
                    };
                    rows.push(DisplayRow {
                        src: Some(j),
                        kind: RowKind::Detailed { base },
                    });
                } else {
                    rows.push(DisplayRow {
                        src: Some(j),
                        kind: RowKind::Spans(blockquote_spans(&lines[j], callout, j == i)),
                    });
                }
            }
            i = end;
            continue;
        }

        if let Some(level) = heading_level(line) {
            rows.push(line_row(lines, i, cursor_line));
            if let Some((ch, style)) = heading_underline(level) {
                let w = heading_text_width(line).clamp(1, width.max(1));
                rows.push(DisplayRow {
                    src: None,
                    kind: RowKind::Spans(vec![(ch.to_string().repeat(w), style)]),
                });
            }
            i += 1;
            continue;
        }

        rows.push(line_row(lines, i, cursor_line));
        i += 1;
    }
    rows
}

fn line_row(lines: &[String], i: usize, cursor_line: usize) -> DisplayRow {
    if i == cursor_line {
        DisplayRow {
            src: Some(i),
            kind: RowKind::Detailed {
                base: theme::text(),
            },
        }
    } else {
        DisplayRow {
            src: Some(i),
            kind: RowKind::Spans(conceal_line(&lines[i])),
        }
    }
}

fn heading_level(line: &str) -> Option<HeadingLevel> {
    let n = line.bytes().take_while(|b| *b == b'#').count();
    if n == 0 || n > 6 || line.as_bytes().get(n) != Some(&b' ') {
        return None;
    }
    Some(match n {
        1 => HeadingLevel::H1,
        2 => HeadingLevel::H2,
        3 => HeadingLevel::H3,
        4 => HeadingLevel::H4,
        5 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    })
}

fn heading_text_width(line: &str) -> usize {
    let text: String = conceal_line(line).into_iter().map(|(t, _)| t).collect();
    UnicodeWidthStr::width(text.as_str())
}

fn is_tableish(line: &str) -> bool {
    line.contains('|') && !line.trim().is_empty()
}

fn is_delimiter(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && t.contains('-')
        && t.contains('|')
        && t.chars().all(|c| matches!(c, '|' | ':' | '-' | ' '))
}

fn is_table_start(lines: &[String], i: usize) -> bool {
    is_tableish(&lines[i])
        && !is_delimiter(&lines[i])
        && lines.get(i + 1).is_some_and(|l| is_delimiter(l))
}

fn table_end(lines: &[String], i: usize) -> usize {
    let mut j = i + 2;
    while j < lines.len() && is_tableish(&lines[j]) {
        j += 1;
    }
    j
}

fn table_grid(block: &[String], width: usize) -> Vec<Vec<(String, Style)>> {
    let src = block.join("\n");
    let rendered = render(&src, width.max(1) as u16, |_| true);
    rendered
        .lines
        .iter()
        .map(line_to_spans)
        .filter(|spans: &Vec<(String, Style)>| !spans.iter().all(|(t, _)| t.trim().is_empty()))
        .collect()
}

fn line_to_spans(line: &Line<'static>) -> Vec<(String, Style)> {
    line.spans
        .iter()
        .map(|s| (s.content.to_string(), s.style))
        .collect()
}

fn blockquote_end(lines: &[String], i: usize) -> usize {
    let mut j = i;
    while j < lines.len() && lines[j].trim_start().starts_with('>') {
        j += 1;
    }
    j
}

fn callout_color(first_line: &str) -> Option<Color> {
    let (_, kind) = parse_callout(first_line.trim_start().trim_start_matches('>').trim_start());
    if kind.is_empty() {
        None
    } else {
        Some(theme::callout(&kind).1)
    }
}

fn parse_callout(content: &str) -> (String, String) {
    if let Some(rest) = content.strip_prefix("[!")
        && let Some(close) = rest.find(']')
    {
        let kind = rest[..close].to_ascii_lowercase();
        let after = rest[close + 1..].trim().to_string();
        return (after, kind);
    }
    (content.to_string(), String::new())
}

fn blockquote_spans(line: &str, callout: Option<Color>, is_title: bool) -> Vec<(String, Style)> {
    let border_color = callout.unwrap_or(theme::MUTED);
    let content = line.trim_start().trim_start_matches('>').trim_start();
    let mut spans = vec![("│ ".to_string(), Style::default().fg(border_color))];
    match callout {
        Some(_) if is_title => {
            let (rest, kind) = parse_callout(content);
            let (icon, color) = theme::callout(&kind);
            let title = if rest.is_empty() {
                capitalize(&kind)
            } else {
                rest
            };
            spans.push((
                format!("{icon} {title}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }
        _ => spans.extend(conceal_line(content)),
    }
    spans
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srcs(lines: &[&str], cursor: usize) -> Vec<DisplayRow> {
        let owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        editor_plan(&owned, cursor, 80, true)
    }

    fn joined(row: &DisplayRow) -> String {
        match &row.kind {
            RowKind::Spans(spans) => spans.iter().map(|(t, _)| t.as_str()).collect(),
            RowKind::Detailed { .. } => String::new(),
        }
    }

    #[test]
    fn h1_gets_an_underline_row_even_on_the_cursor_line() {
        let rows = srcs(&["# Title", "body"], 0);
        assert!(matches!(rows[0].kind, RowKind::Detailed { .. }));
        assert!(rows[1].src.is_none(), "underline is synthetic");
        assert!(joined(&rows[1]).starts_with('═'));
        assert_eq!(joined(&rows[1]).chars().count(), 5, "matches 'Title' width");
    }

    #[test]
    fn h2_underline_off_cursor_line_too() {
        let rows = srcs(&["## Section", "text"], 1);
        let underline = rows.iter().find(|r| joined(r).starts_with('─'));
        assert!(
            underline.is_some(),
            "H2 underline present off the cursor line"
        );
    }

    #[test]
    fn callout_renders_with_bar_and_title() {
        let rows = srcs(&["> [!tip] Heads up", "> body line"], 5);
        let title = joined(&rows[0]);
        assert!(title.contains('│'), "callout bar present");
        assert!(title.contains("Heads up"), "callout title shown");
        assert!(!title.contains("[!tip]"), "kind marker concealed");
    }

    #[test]
    fn callout_on_cursor_line_is_detailed_and_tinted() {
        let rows = srcs(&["> [!warning] careful"], 0);
        assert!(matches!(rows[0].kind, RowKind::Detailed { .. }));
    }

    #[test]
    fn table_off_cursor_becomes_a_grid() {
        let rows = srcs(&["| a | b |", "| - | - |", "| 1 | 2 |", "after"], 3);
        let grid: String = rows
            .iter()
            .map(|r| joined(r))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(grid.contains('┌') && grid.contains('│') && grid.contains('└'));
    }

    #[test]
    fn table_on_cursor_stays_raw_for_editing() {
        let rows = srcs(&["| a | b |", "| - | - |", "| 1 | 2 |"], 2);
        assert!(
            rows.iter()
                .any(|r| matches!(r.kind, RowKind::Detailed { .. }))
        );
        let text: String = rows
            .iter()
            .map(|r| joined(r))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains('┌'), "no grid while editing the table");
    }

    #[test]
    fn fenced_code_is_not_parsed_as_blocks() {
        let rows = srcs(&["```", "| not | a | table |", "# not a heading", "```"], 9);
        let text: String = rows
            .iter()
            .map(|r| joined(r))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains('┌'), "no table grid inside code fence");
        assert!(
            !text.contains('═'),
            "no heading underline inside code fence"
        );
    }
}
