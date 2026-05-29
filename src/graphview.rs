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

pub fn render_constellation(
    vault: &Vault,
    selected_note_idx: Option<usize>,
    area: Rect,
) -> Vec<Line<'static>> {
    render_hyperbolic(vault, selected_note_idx, area)
}

pub fn render_hyperbolic(
    vault: &Vault,
    selected_note_idx: Option<usize>,
    area: Rect,
) -> Vec<Line<'static>> {
    if area.width < 6 || area.height < 4 {
        return vec![Line::from(Span::styled(
            "tight on space, widen the window".to_string(),
            theme::faint(),
        ))];
    }

    let cell_cols = area.width as usize;
    let cell_rows = area.height as usize;
    let dot_w = (cell_cols * 2) as i32;
    let dot_h = (cell_rows * 4) as i32;

    let n = vault.notes.len();
    if n == 0 {
        return vec![Line::from(Span::styled(
            "no notes yet, create one with <Leader>n".to_string(),
            theme::faint(),
        ))];
    }

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

    let focus_global = selected_note_idx.unwrap_or_else(|| {
        let mut best = 0usize;
        for i in 1..n {
            if degree[i] > degree[best] {
                best = i;
            }
        }
        best
    });

    let depth = bfs_depth(n, &edges, focus_global);

    let max_depth = depth.iter().copied().filter_map(|d| d).max().unwrap_or(0).max(1);

    let mut layer_buckets: Vec<Vec<usize>> = vec![Vec::new(); max_depth + 1];
    let mut disconnected: Vec<usize> = Vec::new();
    for i in 0..n {
        match depth[i] {
            Some(d) => layer_buckets[d].push(i),
            None => disconnected.push(i),
        }
    }
    for bucket in &mut layer_buckets {
        bucket.sort_by(|&a, &b| degree[b].cmp(&degree[a]).then(a.cmp(&b)));
    }
    disconnected.sort_by(|&a, &b| degree[b].cmp(&degree[a]).then(a.cmp(&b)));

    let mut pos: Vec<(f32, f32)> = vec![(0.0, 0.0); n];

    pos[focus_global] = (0.0, 0.0);

    for (layer_idx, bucket) in layer_buckets.iter().enumerate().skip(1) {
        if bucket.is_empty() {
            continue;
        }
        let r = (layer_idx as f32 / max_depth as f32 * 1.4).tanh() * 0.92;
        let count = bucket.len();
        for (k, &note_idx) in bucket.iter().enumerate() {
            let theta = (k as f32 + 0.5) * std::f32::consts::TAU / count as f32
                + layer_idx as f32 * 0.31;
            pos[note_idx] = (r * theta.cos(), r * theta.sin());
        }
    }

    if !disconnected.is_empty() {
        let r = 0.95;
        for (k, &note_idx) in disconnected.iter().enumerate() {
            let theta = (k as f32 + 0.5) * std::f32::consts::TAU / disconnected.len() as f32 + 1.1;
            pos[note_idx] = (r * theta.cos(), r * theta.sin());
        }
    }

    let cx = dot_w as f32 / 2.0;
    let cy = dot_h as f32 / 2.0;
    let pixel_aspect_y_per_x = 0.6;
    let radius_dots = (cx.min(cy / pixel_aspect_y_per_x) - 2.0).max(4.0);

    let dot_pos: Vec<(i32, i32)> = pos
        .iter()
        .map(|&(x, y)| {
            (
                (cx + x * radius_dots).round() as i32,
                (cy + y * radius_dots * pixel_aspect_y_per_x).round() as i32,
            )
        })
        .collect();

    let mut canvas = Canvas::new(cell_cols, cell_rows);

    draw_ring(&mut canvas, cx as i32, cy as i32, radius_dots as i32, pixel_aspect_y_per_x);

    for &(i, j) in &edges {
        let (x0, y0) = dot_pos[i];
        let (x1, y1) = dot_pos[j];
        let touches_focus = i == focus_global || j == focus_global;
        let r_avg = ((pos[i].0.powi(2) + pos[i].1.powi(2)).sqrt()
            + (pos[j].0.powi(2) + pos[j].1.powi(2)).sqrt())
            * 0.5;
        if !touches_focus && r_avg > 0.7 {
            draw_line_sparse(&mut canvas, x0, y0, x1, y1, 3);
        } else if !touches_focus && r_avg > 0.45 {
            draw_line_sparse(&mut canvas, x0, y0, x1, y1, 2);
        } else {
            canvas.line(x0, y0, x1, y1);
        }
    }

    for i in 0..n {
        let (x, y) = dot_pos[i];
        let r_norm = (pos[i].0.powi(2) + pos[i].1.powi(2)).sqrt();
        let base_r = if i == focus_global {
            3
        } else if degree[i] >= 6 {
            2
        } else if degree[i] >= 2 {
            1
        } else {
            0
        };
        let shrink = if r_norm > 0.85 { -1 } else if r_norm > 0.6 { 0 } else { 0 };
        let r = (base_r + shrink).max(0);
        if r == 0 {
            canvas.set(x, y);
        } else {
            canvas.fill_disc(x, y, r);
        }
    }

    let mut label_cells: Vec<Vec<Option<(Span<'static>, bool)>>> =
        (0..cell_rows).map(|_| vec![None; cell_cols]).collect();

    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let ra = (pos[a].0.powi(2) + pos[a].1.powi(2)).sqrt();
        let rb = (pos[b].0.powi(2) + pos[b].1.powi(2)).sqrt();
        if a == focus_global {
            return std::cmp::Ordering::Less;
        }
        if b == focus_global {
            return std::cmp::Ordering::Greater;
        }
        ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
            .then(degree[b].cmp(&degree[a]))
    });

    for i in order {
        let (dx, dy) = dot_pos[i];
        let cell_col = dx / 2;
        let cell_row = dy / 4;
        let title = vault
            .notes
            .get(i)
            .map(|nm| nm.title.clone())
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let max_len = (cell_cols / 4).max(8).min(24);
        let r_norm = (pos[i].0.powi(2) + pos[i].1.powi(2)).sqrt();

        let style = if Some(i) == selected_note_idx || i == focus_global {
            theme::brand().add_modifier(Modifier::BOLD)
        } else if r_norm < 0.4 {
            theme::text()
        } else if r_norm < 0.75 {
            theme::muted()
        } else {
            theme::faint()
        };

        let shown = if r_norm > 0.8 {
            truncate(&title, max_len.min(10))
        } else {
            truncate(&title, max_len)
        };

        let want_right = pos[i].0 >= -0.05;
        let label_row = (cell_row + 1).clamp(0, cell_rows as i32 - 1) as usize;
        let len_chars = shown.chars().count() as i32;
        let primary_col = if want_right {
            (cell_col + 2).clamp(0, cell_cols as i32 - len_chars)
        } else {
            (cell_col - len_chars - 1).max(0)
        };
        if !place_label(&mut label_cells, label_row, primary_col as usize, &shown, style.clone()) {
            let alt_col = if want_right {
                (cell_col - len_chars - 1).max(0)
            } else {
                (cell_col + 2).clamp(0, cell_cols as i32 - len_chars)
            };
            let alt_row = (cell_row - 1).clamp(0, cell_rows as i32 - 1) as usize;
            if !place_label(&mut label_cells, alt_row, alt_col as usize, &shown, style.clone()) {
                place_label(&mut label_cells, label_row, alt_col as usize, &shown, style);
            }
        }
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

fn bfs_depth(n: usize, edges: &[(usize, usize)], start: usize) -> Vec<Option<usize>> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b) in edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    let mut depth = vec![None; n];
    let mut q = std::collections::VecDeque::new();
    depth[start] = Some(0);
    q.push_back(start);
    while let Some(u) = q.pop_front() {
        let d = depth[u].unwrap();
        for &v in &adj[u] {
            if depth[v].is_none() {
                depth[v] = Some(d + 1);
                q.push_back(v);
            }
        }
    }
    depth
}

fn draw_ring(canvas: &mut Canvas, cx: i32, cy: i32, r: i32, aspect: f32) {
    let steps = 360 / 2;
    for i in 0..steps {
        let theta = (i as f32) * std::f32::consts::TAU / steps as f32;
        let x = cx + (theta.cos() * r as f32).round() as i32;
        let y = cy + (theta.sin() * r as f32 * aspect).round() as i32;
        canvas.set(x, y);
    }
}

fn draw_line_sparse(canvas: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, stride: i32) {
    let dx = (x1 - x0).abs();
    let dy = -((y1 - y0).abs());
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);
    let mut step = 0i32;
    loop {
        if step % stride == 0 {
            canvas.set(x, y);
        }
        step += 1;
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
