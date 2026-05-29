use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;
use crate::vault::Vault;

const BRAILLE_BASE: u32 = 0x2800;

const DOT_BITS: [u32; 8] = [
    0x01, 0x08,
    0x02, 0x10,
    0x04, 0x20,
    0x40, 0x80,
];

pub struct Canvas {
    cell_cols: usize,
    cell_rows: usize,
    cells: Vec<u32>,
}

impl Canvas {
    pub fn new(cell_cols: usize, cell_rows: usize) -> Self {
        Self {
            cell_cols,
            cell_rows,
            cells: vec![0; cell_cols * cell_rows],
        }
    }

    pub fn set(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.cell_cols * 2 || y >= self.cell_rows * 4 {
            return;
        }
        let cx = x / 2;
        let cy = y / 4;
        let dx = x % 2;
        let dy = y % 4;
        let bit = DOT_BITS[dy * 2 + dx];
        self.cells[cy * self.cell_cols + cx] |= bit;
    }

    pub fn cell(&self, col: usize, row: usize) -> char {
        let bits = self.cells[row * self.cell_cols + col];
        char::from_u32(BRAILLE_BASE + bits).unwrap_or(' ')
    }
}

pub fn render_constellation(
    vault: &Vault,
    selected_note_idx: Option<usize>,
    area: Rect,
) -> Vec<Line<'static>> {
    render_sankey(vault, selected_note_idx, area)
}

pub fn render_sankey(
    vault: &Vault,
    selected_note_idx: Option<usize>,
    area: Rect,
) -> Vec<Line<'static>> {
    if area.width < 30 || area.height < 6 {
        return vec![Line::from(Span::styled(
            "tight on space, widen the window".to_string(),
            theme::faint(),
        ))];
    }

    let cell_cols = area.width as usize;
    let cell_rows = area.height as usize;

    let n = vault.notes.len();
    if n == 0 {
        return vec![Line::from(Span::styled(
            "no notes yet, create one with <Leader>n".to_string(),
            theme::faint(),
        ))];
    }

    let focus_global = selected_note_idx.unwrap_or_else(|| {
        let mut best = 0usize;
        let mut best_deg = 0usize;
        for i in 0..n {
            let d = vault.backlinks(i).len() + vault.outbound(i).len();
            if d > best_deg {
                best_deg = d;
                best = i;
            }
        }
        best
    });

    let mut backlinks: Vec<usize> = vault.backlinks(focus_global).to_vec();
    backlinks.sort_by_key(|&i| {
        std::cmp::Reverse(vault.backlinks(i).len() + vault.outbound(i).len())
    });

    let mut outbound: Vec<usize> = vault.outbound(focus_global);
    outbound.sort_by_key(|&i| {
        std::cmp::Reverse(vault.backlinks(i).len() + vault.outbound(i).len())
    });

    let side_cap = cell_rows.saturating_sub(2).max(1);
    backlinks.truncate(side_cap);
    outbound.truncate(side_cap);

    let label_w = ((cell_cols.saturating_sub(20)) / 3).clamp(8, 26);

    let left_col_end = label_w as i32;
    let right_col_start = cell_cols as i32 - label_w as i32;
    let center_col = cell_cols as i32 / 2;
    let center_row = cell_rows as i32 / 2;

    let dot_w = (cell_cols * 2) as i32;
    let dot_h = (cell_rows * 4) as i32;
    let left_anchor_x = (left_col_end + 1) * 2;
    let right_anchor_x = (right_col_start - 2) * 2;
    let center_anchor_x_left = (center_col - 4) * 2;
    let center_anchor_x_right = (center_col + 4) * 2;
    let center_anchor_y = center_row * 4 + 2;

    let mut canvas = Canvas::new(cell_cols, cell_rows);

    let plot_curve = |canvas: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32| {
        let steps = ((x1 - x0).abs() + (y1 - y0).abs()).max(8) as usize;
        let mid_x = (x0 + x1) / 2;
        let p1x = mid_x as f32;
        let p1y = y0 as f32;
        let p2x = mid_x as f32;
        let p2y = y1 as f32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let one_t = 1.0 - t;
            let x = one_t * one_t * one_t * x0 as f32
                + 3.0 * one_t * one_t * t * p1x
                + 3.0 * one_t * t * t * p2x
                + t * t * t * x1 as f32;
            let y = one_t * one_t * one_t * y0 as f32
                + 3.0 * one_t * one_t * t * p1y
                + 3.0 * one_t * t * t * p2y
                + t * t * t * y1 as f32;
            let xi = x.round() as i32;
            let yi = y.round() as i32;
            if xi >= 0 && yi >= 0 && xi < dot_w && yi < dot_h {
                canvas.set(xi, yi);
            }
        }
    };

    let mut left_rows: Vec<(usize, i32, i32)> = Vec::new();
    if !backlinks.is_empty() {
        let n_left = backlinks.len() as i32;
        let usable_h = (cell_rows as i32 - 2).max(1);
        for (k, &note_idx) in backlinks.iter().enumerate() {
            let row = 1 + ((k as i32) * usable_h) / n_left.max(1);
            let dot_y = row * 4 + 2;
            left_rows.push((note_idx, row, dot_y));
            plot_curve(&mut canvas, left_anchor_x, dot_y, center_anchor_x_left, center_anchor_y);
        }
    }

    let mut right_rows: Vec<(usize, i32, i32)> = Vec::new();
    if !outbound.is_empty() {
        let n_right = outbound.len() as i32;
        let usable_h = (cell_rows as i32 - 2).max(1);
        for (k, &note_idx) in outbound.iter().enumerate() {
            let row = 1 + ((k as i32) * usable_h) / n_right.max(1);
            let dot_y = row * 4 + 2;
            right_rows.push((note_idx, row, dot_y));
            plot_curve(&mut canvas, center_anchor_x_right, center_anchor_y, right_anchor_x, dot_y);
        }
    }

    let mut label_cells: Vec<Vec<Option<(Span<'static>, bool)>>> =
        (0..cell_rows).map(|_| vec![None; cell_cols]).collect();

    let title_focus = vault
        .notes
        .get(focus_global)
        .map(|n| n.title.clone())
        .unwrap_or_default();
    let bracketed = format!("● {} ●", truncate(&title_focus, 26));
    let center_label_w = bracketed.chars().count() as i32;
    let center_col_start = (center_col - center_label_w / 2).max(0) as usize;
    let center_row_u = center_row as usize;
    let center_row_clamped = center_row_u.min(cell_rows.saturating_sub(1));
    place_label(
        &mut label_cells,
        center_row_clamped,
        center_col_start,
        &bracketed,
        theme::brand().add_modifier(Modifier::BOLD),
    );

    let header_row = center_row_clamped.saturating_sub(1);
    let header = format!("({} in)   (out {})", backlinks.len(), outbound.len());
    let header_w = header.chars().count() as i32;
    let header_col = (center_col - header_w / 2).max(0) as usize;
    place_label(&mut label_cells, header_row, header_col, &header, theme::faint());

    for (note_idx, row, _dot_y) in &left_rows {
        let title = vault
            .notes
            .get(*note_idx)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        let shown = truncate(&title, label_w);
        let len = shown.chars().count() as i32;
        let col_start = (left_col_end - 1 - len).max(0) as usize;
        let is_selected = selected_note_idx.is_some() && Some(*note_idx) == selected_note_idx;
        let style = if is_selected {
            theme::brand().add_modifier(Modifier::BOLD)
        } else {
            theme::muted()
        };
        place_label(&mut label_cells, *row as usize, col_start, &shown, style);
    }

    for (note_idx, row, _dot_y) in &right_rows {
        let title = vault
            .notes
            .get(*note_idx)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        let shown = truncate(&title, label_w);
        let col_start = (right_col_start + 2).clamp(0, cell_cols as i32 - 1) as usize;
        let is_selected = selected_note_idx.is_some() && Some(*note_idx) == selected_note_idx;
        let style = if is_selected {
            theme::brand().add_modifier(Modifier::BOLD)
        } else {
            theme::muted()
        };
        place_label(&mut label_cells, *row as usize, col_start, &shown, style);
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(cell_rows);
    for row in 0..cell_rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;
        while col < cell_cols {
            if let Some((label_span, is_label_head)) = label_cells[row][col].clone() {
                if is_label_head {
                    spans.push(label_span.clone());
                    let label_len: usize = label_span.content.chars().count();
                    col += label_len;
                    continue;
                }
            }
            let mut chunk = String::new();
            while col < cell_cols && label_cells[row][col].is_none() {
                chunk.push(canvas.cell(col, row));
                col += 1;
            }
            if !chunk.is_empty() {
                spans.push(Span::styled(chunk, theme::faint()));
            }
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn place_label(
    label_cells: &mut [Vec<Option<(Span<'static>, bool)>>],
    row: usize,
    col_start: usize,
    text: &str,
    style: Style,
) -> bool {
    if row >= label_cells.len() {
        return false;
    }
    let len = text.chars().count();
    if col_start + len > label_cells[row].len() {
        return false;
    }
    for k in 0..len {
        if label_cells[row][col_start + k].is_some() {
            return false;
        }
    }
    label_cells[row][col_start] = Some((Span::styled(text.to_string(), style), true));
    for k in 1..len {
        label_cells[row][col_start + k] = Some((Span::raw(""), false));
    }
    true
}

fn truncate(s: &str, max: usize) -> String {
    let cnt = s.chars().count();
    if cnt <= max {
        return s.to_string();
    }
    let take = max.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
}
