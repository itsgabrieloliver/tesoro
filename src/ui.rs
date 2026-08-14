use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

use crossterm::event::KeyModifiers;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, EditMode, Focus, LeaderKind, SwitcherMode, ViewMode};
use crate::picker::Picker;
use crate::{markdown, theme};

pub fn draw(f: &mut Frame, app: &mut App) {
    let root = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(f.area());
    let show_panel = app.show_panel && app.open.is_some();
    let show_sidebar = app.show_sidebar;
    let mut constraints: Vec<Constraint> = Vec::new();
    if show_sidebar {
        constraints.push(Constraint::Length(if show_panel { 30 } else { 32 }));
    }
    constraints.push(Constraint::Min(0));
    if show_panel {
        constraints.push(Constraint::Length(32));
    }
    let cols = Layout::horizontal(constraints).split(root[0]);

    let mut idx = 0;
    let sidebar_area = if show_sidebar {
        let a = cols[idx];
        idx += 1;
        Some(a)
    } else {
        None
    };
    let center = cols[idx];
    idx += 1;
    let panel_area = if show_panel { Some(cols[idx]) } else { None };

    if let Some(a) = sidebar_area {
        draw_sidebar(f, app, a);
    }

    let (bar_area, content) = if app.open_buffers().is_empty() {
        (None, center)
    } else {
        let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(center);
        (Some(rows[0]), rows[1])
    };
    if let Some(a) = bar_area {
        draw_buffer_bar(f, app, a);
    }

    ensure_render(app, content.width.saturating_sub(2));
    draw_center(f, app, content);

    if let Some(a) = panel_area {
        draw_panel(f, app, a);
    }

    draw_status(f, app, root[1]);

    if app.switcher.is_some() {
        draw_switcher(f, app, f.area());
    }
    if app.search.is_some() {
        draw_search(f, app, f.area());
    }
    if app.prompt.is_some() {
        draw_prompt(f, app, f.area());
    }
    if app.palette.is_some() {
        draw_palette(f, app, f.area());
    }
    if app.graph.is_some() {
        draw_graph(f, app, f.area());
    }
    if app.slash.is_some() {
        draw_slash(f, app, f.area());
    }
}

fn draw_slash(f: &mut Frame, app: &App, area: Rect) {
    let Some(menu) = &app.slash else {
        return;
    };
    let item_count = menu.items.len().max(1);
    let panel_h: u16 = (item_count as u16 + 2).min(10);
    let panel_w: u16 = 44.min(area.width.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(panel_w + 2);
    let y = area.y + area.height.saturating_sub(panel_h + 2);
    let rect = Rect { x, y, width: panel_w, height: panel_h };
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_style(theme::brand())
        .title(format!(" /{} ", menu.filter));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    if menu.items.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            "no match".to_string(),
            theme::faint(),
        )));
        f.render_widget(p, inner);
        return;
    }
    let rows: Vec<Line> = menu
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let style = if i == menu.sel { theme::selected() } else { theme::text() };
            let muted = if i == menu.sel { theme::brand() } else { theme::muted() };
            Line::from(vec![
                Span::styled(format!(" /{:<10}", it.label), style),
                Span::styled(it.description.to_string(), muted),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(rows), inner);
}

fn ensure_render(app: &mut App, width: u16) {
    let App { vault, open, .. } = app;
    if let Some(o) = open.as_mut()
        && matches!(o.view, ViewMode::Preview)
        && (o.render.is_none() || o.width != width)
    {
        let src = o.textarea.lines().join("\n");
        let rendered = markdown::render(&src, width, |t| vault.resolve(t).is_some());
        o.render = Some(std::rc::Rc::new(rendered));
        o.width = width;
    }
}

fn draw_buffer_bar(f: &mut Frame, app: &App, area: Rect) {
    let tabs = app.open_buffers();
    let mut spans: Vec<Span> = Vec::new();
    for t in &tabs {
        let title = app
            .vault
            .notes
            .get(t.idx)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        let label = if t.dirty {
            format!(" ●{title} ")
        } else {
            format!(" {title} ")
        };
        let style = if t.active {
            theme::selected()
        } else if t.dirty {
            theme::warn()
        } else {
            theme::muted()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::statusbar()),
        area,
    );
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let order = app.display_order();
    let items: Vec<ListItem> = order
        .iter()
        .filter_map(|&i| app.vault.notes.get(i).map(|n| (i, n)))
        .map(|(i, n)| {
            let pinned = app.pinned.contains(&n.path);
            let dirty = app.is_note_dirty(i);
            let mut spans: Vec<Span> = Vec::new();
            if dirty {
                spans.push(Span::styled("● ", theme::warn()));
            }
            if pinned {
                spans.push(Span::styled(format!("* {}", n.title), theme::brand()));
            } else {
                spans.push(Span::styled(n.title.clone(), theme::text()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let border = if app.focus == Focus::Sidebar {
        theme::brand()
    } else {
        theme::border()
    };
    let list = List::new(items)
        .block(
            Block::bordered()
                .border_style(border)
                .title(format!(" notes ({}) ", app.vault.notes.len())),
        )
        .highlight_style(theme::selected());

    let mut state = ListState::default();
    if !app.vault.notes.is_empty() {
        let display_pos = order
            .iter()
            .position(|&i| i == app.selected)
            .unwrap_or(0);
        state.select(Some(display_pos));
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_center(f: &mut Frame, app: &mut App, area: Rect) {
    let view = app.open.as_ref().map(|o| o.view);
    match view {
        None => draw_home(f, app, area),
        Some(ViewMode::Preview) => draw_preview(f, app, area),
        Some(ViewMode::Edit) => draw_editor(f, app, area),
    }
}

fn draw_home(f: &mut Frame, app: &App, area: Rect) {
    let border = if app.focus == Focus::Reader {
        theme::brand()
    } else {
        theme::border()
    };
    let block = Block::bordered().border_style(border).title(" home ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width;
    let toc = app.generate_toc();
    let rendered = markdown::render(&toc, width, |t| app.vault.resolve(t).is_some());
    let para = Paragraph::new(rendered.lines).style(theme::text());
    f.render_widget(para, inner);
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let border = if app.focus == Focus::Reader {
        theme::brand()
    } else {
        theme::border()
    };
    let Some(o) = &app.open else {
        return;
    };
    let title = app
        .vault
        .notes
        .get(o.idx)
        .map(|n| n.title.clone())
        .unwrap_or_default();
    let mut lines = o
        .render
        .as_ref()
        .map(|r| r.lines.clone())
        .unwrap_or_default();
    if let Some(r) = o.render.as_ref()
        && let Some(link) = r.links.get(o.link_sel)
        && let Some(line) = lines.get_mut(link.row)
        && let Some(span) = line.spans.get_mut(link.span_idx)
    {
        span.style = theme::link_selected();
    }
    let para = Paragraph::new(lines)
        .block(
            Block::bordered()
                .border_style(border)
                .title(format!(" {title}  [preview] ")),
        )
        .style(theme::text())
        .scroll((o.scroll, 0));
    f.render_widget(para, area);
}

fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Reader;
    let border = if focused {
        theme::brand()
    } else {
        theme::border()
    };
    let (title_str, mode_label, dirty) = {
        let o = app.open.as_ref().unwrap();
        let t = app
            .vault
            .notes
            .get(o.idx)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        let label = match o.mode {
            EditMode::Normal => "NORMAL",
            EditMode::Insert => "INSERT",
            EditMode::Visual => "VISUAL",
        };
        (t, label, o.dirty)
    };
    let border = if dirty { theme::warn() } else { border };
    let title_line = if dirty {
        Line::from(vec![
            Span::styled(" ● ", theme::warn().add_modifier(Modifier::BOLD)),
            Span::styled(title_str, theme::warn().add_modifier(Modifier::BOLD)),
            Span::styled("  [", theme::muted()),
            Span::styled(mode_label, theme::muted()),
            Span::styled("]  ", theme::muted()),
            Span::styled("UNSAVED", theme::warn().add_modifier(Modifier::BOLD | Modifier::REVERSED)),
            Span::raw(" "),
        ])
    } else {
        Line::from(vec![Span::raw(format!(" {title_str}  [{mode_label}] "))])
    };
    let block = Block::bordered().border_style(border).title(title_line);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(o) = app.open.as_mut() else {
        return;
    };
    let (cr, cc) = o.textarea.cursor();
    let selection = o.textarea.selection_range();
    let mode = o.mode;
    let conceal = selection.is_none() && matches!(mode, EditMode::Normal | EditMode::Insert);

    let lines: Vec<String> = o.textarea.lines().to_vec();
    let plan = markdown::editor_plan(&lines, cr, inner.width as usize, conceal);

    let cursor_drow = plan
        .iter()
        .position(|row| {
            row.src == Some(cr) && matches!(row.kind, markdown::RowKind::Detailed { .. })
        })
        .or_else(|| plan.iter().position(|row| row.src == Some(cr)))
        .unwrap_or(0) as u16;
    if cursor_drow < o.editor_top {
        o.editor_top = cursor_drow;
    }
    if inner.height > 0 && cursor_drow >= o.editor_top + inner.height {
        o.editor_top = cursor_drow + 1 - inner.height;
    }
    let top = o.editor_top as usize;

    let buf = f.buffer_mut();
    let mut cursor_screen: Option<(u16, u16)> = None;

    let row_count = (inner.height as usize).min(plan.len().saturating_sub(top));
    for r in 0..row_count {
        let row = &plan[top + r];
        let y = inner.y + r as u16;
        match &row.kind {
            markdown::RowKind::Spans(spans) => {
                let mut x = inner.x;
                for (txt, st) in spans {
                    buf.set_string(x, y, txt, *st);
                    x += UnicodeWidthStr::width(txt.as_str()) as u16;
                }
            }
            markdown::RowKind::Detailed { base } => {
                let src = row.src.unwrap_or(0);
                let is_cursor = row.src == Some(cr);
                paint_source_line(
                    buf,
                    inner,
                    y,
                    &lines[src],
                    is_cursor,
                    cc,
                    *base,
                    &mut cursor_screen,
                );
            }
        }
    }

    if let Some((s, e)) = selection {
        let (start, end) = if (s.0, s.1) <= (e.0, e.1) { (s, e) } else { (e, s) };
        for ry in start.0..=end.0 {
            if ry < top || ry >= top + inner.height as usize {
                continue;
            }
            let y = inner.y + (ry - top) as u16;
            let line_len = lines.get(ry).map(|l| l.chars().count()).unwrap_or(0);
            let col_start = if ry == start.0 { start.1 } else { 0 };
            let col_end = if ry == end.0 { end.1 } else { line_len };
            for cx in col_start..col_end {
                if (cx as u16) < inner.width {
                    let x = inner.x + cx as u16;
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(theme::link_selected());
                    }
                }
            }
        }
    }

    if focused
        && let Some((cx, cy)) = cursor_screen
    {
        f.set_cursor_position(ratatui::layout::Position { x: cx, y: cy });
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_source_line(
    buf: &mut Buffer,
    inner: Rect,
    y: u16,
    line: &str,
    is_cursor: bool,
    cc: usize,
    base: Style,
    cursor_screen: &mut Option<(u16, u16)>,
) {
    let chars: Vec<char> = line.chars().collect();
    let links = markdown::wikilink_positions(line);
    let mut visual_col: u16 = 0;
    let mut last_end: usize = 0;

    for (bs, be, _target) in &links {
        if *bs > last_end {
            if is_cursor && cc >= last_end && cc < *bs && cursor_screen.is_none() {
                *cursor_screen = Some((inner.x + visual_col + (cc - last_end) as u16, y));
            }
            let segment: String = chars[last_end..*bs].iter().collect();
            buf.set_string(inner.x + visual_col, y, &segment, base);
            visual_col += (*bs - last_end) as u16;
        }
        let raw: String = chars[*bs..*be].iter().collect();
        let cursor_in_link = is_cursor && cc >= *bs && cc < *be;
        let link_style = if cursor_in_link {
            theme::link_selected()
        } else {
            theme::link()
        };
        buf.set_string(inner.x + visual_col, y, &raw, link_style);
        if cursor_in_link {
            *cursor_screen = Some((inner.x + visual_col + (cc - *bs) as u16, y));
        }
        visual_col += (*be - *bs) as u16;
        last_end = *be;
    }

    if is_cursor && cc >= last_end && cursor_screen.is_none() {
        *cursor_screen = Some((inner.x + visual_col + (cc - last_end) as u16, y));
    }
    if last_end < chars.len() {
        let trailing: String = chars[last_end..].iter().collect();
        buf.set_string(inner.x + visual_col, y, &trailing, base);
    }
}

fn draw_panel(f: &mut Frame, app: &App, area: Rect) {
    let Some(o) = &app.open else {
        return;
    };
    let titles = |idxs: Vec<usize>| -> Vec<String> {
        idxs.into_iter()
            .filter_map(|i| app.vault.notes.get(i))
            .map(|n| n.title.clone())
            .collect()
    };
    let back = titles(app.vault.backlinks(o.idx).to_vec());
    let out = titles(app.vault.outbound(o.idx));

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("Backlinks ({})", back.len()),
        theme::brand(),
    )));
    if back.is_empty() {
        lines.push(Line::from(Span::styled("  none", theme::faint())));
    }
    for t in &back {
        lines.push(Line::from(Span::styled(format!("  {t}"), theme::link())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("Links ({})", out.len()),
        theme::brand(),
    )));
    if out.is_empty() {
        lines.push(Line::from(Span::styled("  none", theme::faint())));
    }
    for t in &out {
        lines.push(Line::from(Span::styled(format!("  {t}"), theme::link())));
    }

    let para = Paragraph::new(lines).block(
        Block::bordered()
            .border_style(theme::border())
            .title(" graph "),
    );
    f.render_widget(para, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let name = app
        .vault
        .root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("vault");
    let mode = if app.focus == Focus::Sidebar {
        "sidebar".to_string()
    } else {
        match app.open.as_ref().map(|o| (o.view, o.mode)) {
            Some((ViewMode::Edit, EditMode::Normal)) => "NORMAL".to_string(),
            Some((ViewMode::Edit, EditMode::Insert)) => "INSERT".to_string(),
            Some((ViewMode::Edit, EditMode::Visual)) => "VISUAL".to_string(),
            Some((ViewMode::Preview, _)) => "preview".to_string(),
            None => "note".to_string(),
        }
    };
    let leader_label = match app.leader_kind {
        LeaderKind::Modifier(KeyModifiers::CONTROL) => "ctrl".to_string(),
        LeaderKind::Modifier(KeyModifiers::ALT) => "alt".to_string(),
        LeaderKind::Modifier(_) => "mod".to_string(),
        LeaderKind::Char(' ') => "space".to_string(),
        LeaderKind::Char(c) => format!("'{c}'"),
    };
    let mut spans = vec![
        Span::styled(" tesoro ", theme::brand()),
        Span::styled(format!(" {name} "), theme::muted()),
        Span::styled(format!(" {mode} "), theme::muted()),
        Span::styled(format!(" leader:{leader_label} "), theme::muted()),
        Span::styled(
            " enter:follow  :w save  ldr+[/]:back/fwd  ldr+`:back  ^l:make-link  ^p:cmds ",
            theme::faint(),
        ),
    ];
    if app.leader_pending {
        spans.push(Span::styled(" [pending] ", theme::brand()));
    }
    if let Some(s) = &app.status {
        spans.push(Span::styled(format!(" {s} "), theme::brand()));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::statusbar()),
        area,
    );
}

fn draw_picker_modal(
    f: &mut Frame,
    area: Rect,
    title: &str,
    p: &Picker,
    label: impl Fn(usize) -> String,
) {
    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);

    let block = Block::bordered()
        .border_style(theme::brand())
        .title(format!(" {title} "));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(inner);
    let query = Line::from(vec![
        Span::styled("› ", theme::muted()),
        Span::styled(p.query.as_str(), theme::text()),
    ]);
    f.render_widget(Paragraph::new(query), rows[0]);

    let height = rows[1].height as usize;
    let start = p.sel.saturating_sub(height.saturating_sub(1));
    let items: Vec<ListItem> = p
        .matches
        .iter()
        .skip(start)
        .take(height)
        .enumerate()
        .map(|(vis, &mi)| {
            let abs = start + vis;
            let style = if abs == p.sel {
                theme::selected()
            } else {
                theme::text()
            };
            ListItem::new(Line::from(Span::styled(label(mi), style)))
        })
        .collect();
    f.render_widget(List::new(items), rows[1]);
}

fn draw_switcher(f: &mut Frame, app: &App, area: Rect) {
    let Some(p) = &app.switcher else {
        return;
    };
    let tags_mode = app.switcher_mode == SwitcherMode::Tags;
    let title = match app.switcher_mode {
        SwitcherMode::Tags => "tags",
        SwitcherMode::LinkOrCreate => "link or create",
        SwitcherMode::Notes => "quick switch",
    };
    draw_picker_modal(f, area, title, p, |mi| {
        if tags_mode {
            format!(
                "{}  ({})",
                p.items[mi],
                app.vault.notes_with_tag(&p.items[mi]).len()
            )
        } else {
            p.items[mi].clone()
        }
    });
}

fn draw_palette(f: &mut Frame, app: &App, area: Rect) {
    let Some(p) = &app.palette else {
        return;
    };
    draw_picker_modal(f, area, "commands", p, |mi| p.items[mi].clone());
}

fn draw_graph(f: &mut Frame, app: &App, area: Rect) {
    let Some(g) = &app.graph else {
        return;
    };
    f.render_widget(Clear, area);
    let root_title = app
        .vault
        .notes
        .get(g.root_idx)
        .map(|n| n.title.as_str())
        .unwrap_or("?")
        .to_string();
    let title = format!(
        " tree - {root_title}  (↑↓ move  →/l expand  ←/h collapse  enter:open  esc:close) "
    );
    let block = Block::bordered().border_style(theme::brand()).title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = crate::graphview::render_tree(&app.vault, &g.visible, &g.expanded, g.sel, inner);
    let para = Paragraph::new(lines).style(theme::text());
    f.render_widget(para, inner);
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let Some(s) = &app.search else {
        return;
    };
    let popup = centered_rect(70, 70, area);
    f.render_widget(Clear, popup);

    let block = Block::bordered()
        .border_style(theme::brand())
        .title(format!(" search ({}) ", s.results.len()));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(inner);
    let query = Line::from(vec![
        Span::styled("/ ", theme::muted()),
        Span::styled(s.query.as_str(), theme::text()),
    ]);
    f.render_widget(Paragraph::new(query), rows[0]);

    let height = rows[1].height as usize;
    let start = s.sel.saturating_sub(height.saturating_sub(1));
    let items: Vec<ListItem> = s
        .results
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(abs, (ni, snippet))| {
            let title = app
                .vault
                .notes
                .get(*ni)
                .map(|n| n.title.as_str())
                .unwrap_or("?");
            let style = if abs == s.sel {
                theme::selected()
            } else {
                theme::text()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{title}  "), style),
                Span::styled(snippet.as_str(), theme::muted()),
            ]))
        })
        .collect();
    f.render_widget(List::new(items), rows[1]);
}

fn draw_prompt(f: &mut Frame, app: &App, area: Rect) {
    let Some(p) = &app.prompt else {
        return;
    };
    let popup = centered_rect(50, 20, area);
    f.render_widget(Clear, popup);
    let block = Block::bordered()
        .border_style(theme::brand())
        .title(format!(" {} ", p.title));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let line = Line::from(vec![
        Span::styled("› ", theme::muted()),
        Span::styled(p.input.as_str(), theme::text()),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn centered_rect(px: u16, py: u16, area: Rect) -> Rect {
    let w = area.width * px / 100;
    let h = area.height * py / 100;
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn buffer_text(buf: &Buffer) -> String {
        let area = buf.area();
        let mut s = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                s.push_str(buf[(x, y)].symbol());
            }
        }
        s
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn load(dir: &std::path::Path) -> App {
        App::new(Vault::load(dir.to_path_buf()).unwrap())
    }

    #[test]
    fn opens_a_note_into_the_editor() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Hello.md"),
            "# Hello\n\nuniquebodymarker [[World]]\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("World.md"), "# World\n").unwrap();
        let mut app = load(dir.path());

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let before = buffer_text(terminal.backend().buffer());
        assert!(before.contains("notes"));
        assert!(before.contains("home"));

        app.on_key(key(KeyCode::Enter));
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let after = buffer_text(terminal.backend().buffer());
        assert!(after.contains("uniquebodymarker"));
        assert!(after.contains("NORMAL"));
        assert!(!after.contains("home"));
    }

    #[test]
    fn down_arrow_jumps_to_next_link() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("A.md"),
            "intro\n\nsee [[One]] then [[Two]]\n",
        )
        .unwrap();
        let mut app = load(dir.path());
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Down));
        let (r, c) = app.open.as_ref().unwrap().textarea.cursor();
        assert_eq!((r, c), (2, 4));
    }

    #[test]
    fn following_a_phantom_link_creates_and_opens_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "# A\n\nlink to [[Ghost]]\n").unwrap();
        let mut app = load(dir.path());

        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Down));
        app.on_key(key(KeyCode::Enter));
        assert!(dir.path().join("Ghost.md").exists());
        let idx = app.open.as_ref().unwrap().idx;
        assert_eq!(app.vault.notes[idx].title, "Ghost");
    }

    #[test]
    fn edits_in_insert_mode_then_save() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("A.md");
        std::fs::write(&p, "start\n").unwrap();
        let mut app = load(dir.path());
        app.on_key(key(KeyCode::Enter)); // open editor (normal)
        app.on_key(key(KeyCode::Char('A'))); // append at end of line -> insert
        for c in " more".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)); // save
        let saved = std::fs::read_to_string(&p).unwrap();
        assert!(saved.contains("start more"));
    }

    #[test]
    fn switcher_opens_filters_and_opens() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Apple.md"), "# Apple\n").unwrap();
        std::fs::write(dir.path().join("Banana.md"), "# Banana\n").unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        app.on_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
        assert!(app.switcher.is_some());
        app.on_key(key(KeyCode::Char('b')));
        app.on_key(key(KeyCode::Char('a')));
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("quick switch"));

        app.on_key(key(KeyCode::Enter));
        assert!(app.switcher.is_none());
        let idx = app.open.as_ref().unwrap().idx;
        assert_eq!(app.vault.notes[idx].title, "Banana");
    }

    #[test]
    fn external_change_adds_then_removes_notes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("One.md"), "# One\n").unwrap();
        let mut app = load(dir.path());
        assert_eq!(app.vault.notes.len(), 1);

        let two = dir.path().join("Two.md");
        std::fs::write(&two, "# Two\n").unwrap();
        app.on_external_change(&two);
        assert_eq!(app.vault.notes.len(), 2);
        assert!(app.vault.resolve("Two").is_some());

        std::fs::remove_file(&two).unwrap();
        app.on_external_change(&two);
        assert_eq!(app.vault.notes.len(), 1);
    }

    #[test]
    fn search_opens_and_opens_a_hit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "# A\nthe quick brown fox\n").unwrap();
        std::fs::write(dir.path().join("B.md"), "# B\nplain text\n").unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        app.on_key(key(KeyCode::Char('/')));
        for c in "brown".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("search"));
        assert!(text.contains("brown"));

        app.on_key(key(KeyCode::Enter));
        assert!(app.search.is_none());
        assert_eq!(app.vault.notes[app.open.as_ref().unwrap().idx].title, "A");
    }

    #[test]
    fn panel_shows_backlinks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Target.md"), "# Target\n").unwrap();
        std::fs::write(
            dir.path().join("Source.md"),
            "# Source\nlinks [[Target]]\n",
        )
        .unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();

        app.on_key(key(KeyCode::Char('j')));
        app.on_key(key(KeyCode::Enter));
        app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("Backlinks"));
        assert!(text.contains("graph"));
    }

    #[test]
    fn tag_browser_drills_to_tagged_notes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "# A\n#proj\n").unwrap();
        std::fs::write(dir.path().join("B.md"), "# B\nplain\n").unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        app.on_key(key(KeyCode::Char('t')));
        assert!(app.switcher.is_some());
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("tags"));

        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Enter));
        let idx = app.open.as_ref().unwrap().idx;
        assert_eq!(app.vault.notes[idx].title, "A");
    }

    #[test]
    fn new_note_prompt_creates_note() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Seed.md"), "# Seed\n").unwrap();
        let mut app = load(dir.path());

        app.on_key(key(KeyCode::Char('n')));
        assert!(app.prompt.is_some());
        for c in "Zed".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Enter));
        assert!(app.prompt.is_none());
        assert!(dir.path().join("Zed.md").exists());
        assert_eq!(app.vault.notes[app.open.as_ref().unwrap().idx].title, "Zed");
    }

    #[test]
    fn palette_open_in_editor_sets_request() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Note.md"), "# Note\n").unwrap();
        let mut app = load(dir.path());
        app.on_key(key(KeyCode::Enter));
        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        for c in "editor".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Enter));
        assert!(app.edit_request.is_some());
    }

    #[test]
    fn buffer_bar_and_sidebar_show_open_and_dirty_buffers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "# A\n").unwrap();
        std::fs::write(dir.path().join("B.md"), "# B\n").unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();

        app.open_path(&dir.path().join("A.md"));
        app.on_key(key(KeyCode::Char('A')));
        for c in "edit".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Esc));
        app.open_path(&dir.path().join("B.md"));

        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains('●'), "a dirty marker should be visible");
        assert!(text.contains('A') && text.contains('B'), "both buffers listed");
    }

    #[test]
    fn editor_conceals_markup_on_non_cursor_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Note.md"),
            "# Big Title\n\nplain **bold** text\n",
        )
        .unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        app.on_key(key(KeyCode::Enter));
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("# Big Title"), "cursor line stays raw");
        assert!(text.contains("bold"));
        assert!(
            !text.contains("**bold**"),
            "markup hidden on non-cursor lines"
        );
    }

    #[test]
    fn graph_view_opens_and_opens_note() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Hub.md"), "# Hub\n").unwrap();
        std::fs::write(dir.path().join("A.md"), "# A\n[[Hub]]\n").unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        assert!(app.graph.is_some());
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("tree"));

        app.on_key(key(KeyCode::Enter));
        assert!(app.graph.is_none());
        let idx = app.open.as_ref().unwrap().idx;
        assert_eq!(app.vault.notes[idx].title, "Hub");
    }
}
