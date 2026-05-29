pub fn format_markdown(src: &str) -> String {
    let raw_lines: Vec<&str> = src.split('\n').collect();
    let trimmed: Vec<String> = raw_lines
        .iter()
        .map(|l| l.trim_end_matches(|c: char| c == ' ' || c == '\t').to_string())
        .collect();

    let mut out: Vec<String> = Vec::with_capacity(trimmed.len());
    let mut i = 0usize;
    while i < trimmed.len() {
        let line = &trimmed[i];
        if is_table_row(line) {
            let mut j = i;
            while j < trimmed.len() && is_table_row(&trimmed[j]) {
                j += 1;
            }
            let block = &trimmed[i..j];
            let formatted = format_table(block);
            out.extend(formatted);
            i = j;
        } else {
            out.push(line.clone());
            i += 1;
        }
    }

    let mut joined = collapse_blank_lines(out);
    while joined.last().map(|s| s.is_empty()).unwrap_or(false) {
        joined.pop();
    }
    let mut text = joined.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn is_table_row(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('|') && t.contains('|') && t.matches('|').count() >= 2
}

fn split_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let inner = t
        .strip_prefix('|')
        .unwrap_or(t)
        .strip_suffix('|')
        .unwrap_or(t);
    inner
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|c| {
            let s = c.trim();
            !s.is_empty()
                && s.chars().all(|ch| matches!(ch, '-' | ':' | ' '))
                && s.contains('-')
        })
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
    Center,
}

fn cell_align(cell: &str) -> Align {
    let s = cell.trim();
    let starts = s.starts_with(':');
    let ends = s.ends_with(':');
    match (starts, ends) {
        (true, true) => Align::Center,
        (false, true) => Align::Right,
        _ => Align::Left,
    }
}

fn cell_marks(cell: &str) -> (bool, bool) {
    let s = cell.trim();
    (s.starts_with(':'), s.ends_with(':'))
}

fn format_table(rows: &[String]) -> Vec<String> {
    let parsed: Vec<Vec<String>> = rows.iter().map(|r| split_cells(r)).collect();
    let col_count = parsed.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return rows.to_vec();
    }

    let sep_idx = parsed.iter().position(|c| is_separator_row(c));

    let aligns: Vec<Align> = if let Some(idx) = sep_idx {
        (0..col_count)
            .map(|c| parsed[idx].get(c).map(|s| cell_align(s)).unwrap_or(Align::Left))
            .collect()
    } else {
        vec![Align::Left; col_count]
    };

    let marks: Vec<(bool, bool)> = if let Some(idx) = sep_idx {
        (0..col_count)
            .map(|c| parsed[idx].get(c).map(|s| cell_marks(s)).unwrap_or((false, false)))
            .collect()
    } else {
        vec![(false, false); col_count]
    };

    let mut widths = vec![0usize; col_count];
    for (i, row) in parsed.iter().enumerate() {
        if Some(i) == sep_idx {
            continue;
        }
        for (c, cell) in row.iter().enumerate() {
            let w = display_width(cell);
            if w > widths[c] {
                widths[c] = w;
            }
        }
    }

    let mut out: Vec<String> = Vec::with_capacity(parsed.len());
    for (i, row) in parsed.iter().enumerate() {
        if Some(i) == sep_idx {
            let mut s = String::new();
            s.push('|');
            for c in 0..col_count {
                let dash_count = widths[c].max(3);
                let bar = "-".repeat(dash_count);
                let (lead_colon, trail_colon) = marks[c];
                let left = if lead_colon { ':' } else { ' ' };
                let right = if trail_colon { ':' } else { ' ' };
                s.push(left);
                s.push_str(&bar);
                s.push(right);
                s.push('|');
            }
            out.push(s);
        } else {
            let mut s = String::new();
            s.push('|');
            for (c, w) in widths.iter().enumerate() {
                let cell = row.get(c).cloned().unwrap_or_default();
                let a = aligns.get(c).copied().unwrap_or(Align::Left);
                let padded = pad_cell(&cell, *w, a);
                s.push(' ');
                s.push_str(&padded);
                s.push(' ');
                s.push('|');
            }
            out.push(s);
        }
    }
    out
}

fn display_width(s: &str) -> usize {
    s.chars().count()
}

fn pad_cell(text: &str, width: usize, align: Align) -> String {
    let cur = display_width(text);
    if cur >= width {
        return text.to_string();
    }
    let pad = width - cur;
    match align {
        Align::Left => format!("{}{}", text, " ".repeat(pad)),
        Align::Right => format!("{}{}", " ".repeat(pad), text),
        Align::Center => {
            let left = pad / 2;
            let right = pad - left;
            format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
        }
    }
}

fn collapse_blank_lines(lines: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut blank_run = 0usize;
    for line in lines {
        if line.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(line);
            }
        } else {
            blank_run = 0;
            out.push(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_trailing_whitespace() {
        let src = "hello   \nworld \t  \n";
        assert_eq!(format_markdown(src), "hello\nworld\n");
    }

    #[test]
    fn ensures_single_trailing_newline() {
        assert_eq!(format_markdown("a"), "a\n");
        assert_eq!(format_markdown("a\n\n\n"), "a\n");
    }

    #[test]
    fn collapses_blank_runs() {
        let src = "a\n\n\n\nb\n";
        assert_eq!(format_markdown(src), "a\n\nb\n");
    }

    #[test]
    fn aligns_simple_table() {
        let src = "| a | bb |\n|---|---|\n| 1 | 22 |\n";
        let out = format_markdown(src);
        let expected = "| a | bb |\n| --- | --- |\n| 1 | 22 |\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn pads_widest_column() {
        let src = "| name | role |\n|---|---|\n| Gabriel | engineer |\n";
        let out = format_markdown(src);
        assert!(out.contains("| name    | role     |"));
        assert!(out.contains("| Gabriel | engineer |"));
    }

    #[test]
    fn preserves_alignment_markers() {
        let src = "| a | b |\n|:--|--:|\n| 1 | 2 |\n";
        let out = format_markdown(src);
        assert!(out.contains(":---"));
        assert!(out.contains("---:"));
    }

    #[test]
    fn leaves_non_table_text_alone() {
        let src = "# Title\n\nparagraph\n";
        assert_eq!(format_markdown(src), "# Title\n\nparagraph\n");
    }
}
