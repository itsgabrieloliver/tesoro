use std::collections::HashSet;

use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::app::TreeRow;
use crate::theme;
use crate::vault::Vault;

pub fn render_tree(
    vault: &Vault,
    visible: &[TreeRow],
    expanded: &HashSet<Vec<usize>>,
    sel: usize,
    area: Rect,
) -> Vec<Line<'static>> {
    if visible.is_empty() {
        return vec![Line::from(Span::styled(
            "nothing to show".to_string(),
            theme::faint(),
        ))];
    }

    let height = area.height as usize;
    let start = if sel >= height {
        sel + 1 - height
    } else {
        0
    };
    let end = (start + height).min(visible.len());

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(end - start);
    for i in start..end {
        let row = &visible[i];
        let mut prefix = String::new();
        if row.depth > 0 {
            let levels = &row.is_last_at_levels;
            for &was_last in &levels[..levels.len() - 1] {
                prefix.push_str(if was_last { "    " } else { "│   " });
            }
            let last_here = *levels.last().unwrap_or(&false);
            prefix.push_str(if last_here { "└── " } else { "├── " });
        }

        let bullet = if row.has_children {
            if expanded.contains(&row.path) {
                "▾"
            } else {
                "▸"
            }
        } else {
            "·"
        };

        let title = vault
            .notes
            .get(row.note_idx)
            .map(|n| n.title.clone())
            .unwrap_or_else(|| "?".to_string());

        let count = if row.has_children {
            let n = vault
                .outbound(row.note_idx)
                .into_iter()
                .filter(|c| !row.path.contains(c))
                .count();
            format!("  ({n})")
        } else {
            String::new()
        };

        let title_style = if i == sel {
            theme::brand().add_modifier(Modifier::BOLD)
        } else if row.depth == 0 {
            theme::text().add_modifier(Modifier::BOLD)
        } else {
            theme::text()
        };

        let cursor_marker = if i == sel { "▌ " } else { "  " };
        let cursor_style = if i == sel {
            theme::brand()
        } else {
            theme::faint()
        };

        let mut spans: Vec<Span<'static>> = vec![
            Span::styled(cursor_marker.to_string(), cursor_style),
            Span::styled(prefix, theme::faint()),
            Span::styled(format!("{bullet} "), theme::muted()),
            Span::styled(title, title_style),
            Span::styled(count, theme::faint()),
        ];

        let back_count = vault.backlinks(row.note_idx).len();
        if back_count > 0 && row.depth == 0 {
            spans.push(Span::styled(
                format!("  ←{back_count}"),
                theme::muted(),
            ));
        }

        lines.push(Line::from(spans));
    }

    lines
}
