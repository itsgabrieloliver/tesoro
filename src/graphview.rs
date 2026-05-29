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

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let dx = (x1 - x0).abs();
        let dy = -((y1 - y0).abs());
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.set(x, y);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn fill_disc(&mut self, cx: i32, cy: i32, r: i32) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    self.set(cx + dx, cy + dy);
                }
            }
        }
    }

    pub fn cell(&self, col: usize, row: usize) -> char {
        let bits = self.cells[row * self.cell_cols + col];
        char::from_u32(BRAILLE_BASE + bits).unwrap_or(' ')
    }
}

#[derive(Clone, Copy)]
struct Pos {
    x: f32,
    y: f32,
}

fn layout(n: usize, edges: &[(usize, usize)]) -> Vec<Pos> {
    if n == 0 {
        return Vec::new();
    }
    let mut pos: Vec<Pos> = (0..n)
        .map(|i| {
            let theta = (i as f32) * std::f32::consts::TAU / (n as f32);
            Pos { x: theta.cos(), y: theta.sin() }
        })
        .collect();

    let k = (1.0_f32 / n as f32).sqrt().max(0.05);
    let iterations = 80usize;

    for it in 0..iterations {
        let t = 0.18 * (1.0 - it as f32 / iterations as f32).max(0.05);
        let mut disp = vec![Pos { x: 0.0, y: 0.0 }; n];

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let dx = pos[i].x - pos[j].x;
                let dy = pos[i].y - pos[j].y;
                let dist = (dx * dx + dy * dy).sqrt().max(1e-3);
                let force = k * k / dist;
                disp[i].x += dx / dist * force;
                disp[i].y += dy / dist * force;
            }
        }

        for &(i, j) in edges {
            let dx = pos[i].x - pos[j].x;
            let dy = pos[i].y - pos[j].y;
            let dist = (dx * dx + dy * dy).sqrt().max(1e-3);
            let force = dist * dist / k;
            disp[i].x -= dx / dist * force;
            disp[i].y -= dy / dist * force;
            disp[j].x += dx / dist * force;
            disp[j].y += dy / dist * force;
        }

        for i in 0..n {
            let d = (disp[i].x * disp[i].x + disp[i].y * disp[i].y).sqrt().max(1e-3);
            pos[i].x += disp[i].x / d * d.min(t);
            pos[i].y += disp[i].y / d * d.min(t);
            pos[i].x = pos[i].x.clamp(-1.5, 1.5);
            pos[i].y = pos[i].y.clamp(-1.5, 1.5);
        }
    }

    pos
}

fn normalize(pos: &[Pos], cols: i32, rows: i32, pad: i32) -> Vec<(i32, i32)> {
    if pos.is_empty() {
        return Vec::new();
    }
    let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
    for p in pos {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }
    let span_x = (max_x - min_x).max(1e-3);
    let span_y = (max_y - min_y).max(1e-3);
    let usable_w = (cols - 2 * pad).max(1) as f32;
    let usable_h = (rows - 2 * pad).max(1) as f32;
    pos.iter()
        .map(|p| {
            let nx = (p.x - min_x) / span_x;
            let ny = (p.y - min_y) / span_y;
            (
                pad + (nx * usable_w) as i32,
                pad + (ny * usable_h) as i32,
            )
        })
        .collect()
}

pub struct GraphLayout {
    pub note_idx: Vec<usize>,
    pub degree: Vec<usize>,
    pub dot_pos: Vec<(i32, i32)>,
    pub cell_cols: usize,
    pub cell_rows: usize,
}

pub fn build_layout(vault: &Vault, area: Rect) -> GraphLayout {
    let cell_cols = area.width as usize;
    let cell_rows = area.height as usize;
    let dot_w = (cell_cols * 2) as i32;
    let dot_h = (cell_rows * 4) as i32;

    let n = vault.notes.len();
    let note_idx: Vec<usize> = (0..n).collect();

    let mut edges: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        for &j in vault.outbound(i).iter() {
            let (a, b) = if i < j { (i, j) } else { (j, i) };
            if a != b && !edges.iter().any(|e| *e == (a, b)) {
                edges.push((a, b));
            }
        }
    }

    let degree: Vec<usize> = (0..n)
        .map(|i| vault.backlinks(i).len() + vault.outbound(i).len())
        .collect();

    let positions = layout(n, &edges);
    let dot_pos = normalize(&positions, dot_w, dot_h, 4);

    GraphLayout {
        note_idx,
        degree,
        dot_pos,
        cell_cols,
        cell_rows,
    }
}

pub fn render_constellation(
    vault: &Vault,
    selected_note_idx: Option<usize>,
    area: Rect,
) -> Vec<Line<'static>> {
    if area.width < 4 || area.height < 3 {
        return vec![Line::from(Span::styled(
            "tight on space — widen the window".to_string(),
            theme::faint(),
        ))];
    }

    let gl = build_layout(vault, area);
    if gl.note_idx.is_empty() {
        return vec![Line::from(Span::styled(
            "no notes yet — create one with <Leader>n".to_string(),
            theme::faint(),
        ))];
    }

    let mut canvas = Canvas::new(gl.cell_cols, gl.cell_rows);

    for i in 0..gl.note_idx.len() {
        for j in (i + 1)..gl.note_idx.len() {
            let out_i = vault.outbound(gl.note_idx[i]);
            let connected =
                out_i.contains(&gl.note_idx[j]) || vault.outbound(gl.note_idx[j]).contains(&gl.note_idx[i]);
            if connected {
                let (x0, y0) = gl.dot_pos[i];
                let (x1, y1) = gl.dot_pos[j];
                canvas.line(x0, y0, x1, y1);
            }
        }
    }

    for i in 0..gl.note_idx.len() {
        let (x, y) = gl.dot_pos[i];
        let deg = gl.degree[i];
        let r = if deg >= 6 { 2 } else if deg >= 2 { 1 } else { 0 };
        if r == 0 {
            canvas.set(x, y);
        } else {
            canvas.fill_disc(x, y, r);
        }
    }

    let mut label_cells: Vec<Vec<Option<(Span<'static>, bool)>>> =
        (0..gl.cell_rows).map(|_| vec![None; gl.cell_cols]).collect();

    let mut indices: Vec<usize> = (0..gl.note_idx.len()).collect();
    indices.sort_by(|&a, &b| gl.degree[b].cmp(&gl.degree[a]));

    for i in indices {
        let (dx, dy) = gl.dot_pos[i];
        let cell_col = (dx / 2) as i32;
        let cell_row = (dy / 4) as i32;
        let note_idx = gl.note_idx[i];
        let title = vault
            .notes
            .get(note_idx)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let max_len = (gl.cell_cols / 3).max(8).min(28);
        let shown = truncate(&title, max_len);

        let is_selected = selected_note_idx == Some(note_idx);
        let style = if is_selected {
            theme::brand().add_modifier(Modifier::BOLD)
        } else if gl.degree[i] >= 4 {
            theme::text()
        } else {
            theme::muted()
        };

        let label_row = (cell_row + 1).max(0).min(gl.cell_rows as i32 - 1) as usize;
        let label_col_start = (cell_col + 1)
            .max(0)
            .min(gl.cell_cols as i32 - shown.chars().count() as i32 - 1) as usize;

        if !place_label(&mut label_cells, label_row, label_col_start, &shown, style.clone()) {
            let alt_col = (cell_col - shown.chars().count() as i32 - 1).max(0) as usize;
            place_label(&mut label_cells, label_row, alt_col, &shown, style);
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(gl.cell_rows);
    for row in 0..gl.cell_rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;
        while col < gl.cell_cols {
            if let Some((label_span, is_label_head)) = label_cells[row][col].clone() {
                if is_label_head {
                    spans.push(label_span.clone());
                    let label_len: usize = label_span.content.chars().count();
                    col += label_len;
                    continue;
                }
            }
            let mut chunk = String::new();
            while col < gl.cell_cols && label_cells[row][col].is_none() {
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
