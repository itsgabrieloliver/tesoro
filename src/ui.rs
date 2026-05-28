use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

use crossterm::event::KeyModifiers;

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

    ensure_render(app, center.width.saturating_sub(2));
    draw_center(f, app, center);

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

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let order = app.display_order();
    let items: Vec<ListItem> = order
        .iter()
        .filter_map(|&i| app.vault.notes.get(i))
        .map(|n| {
            let pinned = app.pinned.contains(&n.path);
            let line = if pinned {
                Line::from(Span::styled(format!("* {}", n.title), theme::brand()))
            } else {
                Line::from(Span::styled(n.title.clone(), theme::text()))
            };
            ListItem::new(line)
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
    let (title_str, mode_label) = {
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
        (t, label)
    };
    let block = Block::bordered()
        .border_style(border)
        .title(format!(" {title_str}  [{mode_label}] "));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(o) = app.open.as_mut() else {
        return;
    };
    let (cr, cc) = o.textarea.cursor();
    let selection = o.textarea.selection_range();
    let mode = o.mode;
    let conceal = mode == EditMode::Normal && selection.is_none();

    let cr_u16 = cr as u16;
    if cr_u16 < o.editor_top {
        o.editor_top = cr_u16;
    }
    if inner.height > 0 && cr_u16 >= o.editor_top + inner.height {
        o.editor_top = cr_u16 + 1 - inner.height;
    }
    let top = o.editor_top as usize;
    let lines: Vec<String> = o.textarea.lines().to_vec();

    let buf = f.buffer_mut();
    let mut cursor_screen: Option<(u16, u16)> = None;

    let row_count = (inner.height as usize).min(lines.len().saturating_sub(top));
    for r in 0..row_count {
        let line_idx = top + r;
        let y = inner.y + r as u16;
        let line = &lines[line_idx];
        let chars: Vec<char> = line.chars().collect();
        let links = crate::markdown::wikilink_positions(line);
        let cursor_on_this_line = line_idx == cr;

        let mut visual_col: u16 = 0;
        let mut last_end: usize = 0;

        for (bs, be, _target) in &links {
            if *bs > last_end {
                if cursor_on_this_line
                    && cc >= last_end
                    && cc < *bs
                    && cursor_screen.is_none()
                {
                    cursor_screen =
                        Some((inner.x + visual_col + (cc - last_end) as u16, y));
                }
                let segment: String = chars[last_end..*bs].iter().collect();
                buf.set_string(inner.x + visual_col, y, &segment, theme::text());
                visual_col += (*bs - last_end) as u16;
            }
            let raw: String = chars[*bs..*be].iter().collect();
            let cursor_in_link = cursor_on_this_line && cc >= *bs && cc < *be;
            let show_raw = !conceal || cursor_in_link;
            let link_style = if cursor_in_link {
                theme::link_selected()
            } else {
                theme::link()
            };

            if show_raw {
                buf.set_string(inner.x + visual_col, y, &raw, link_style);
                let raw_w = (*be - *bs) as u16;
                if cursor_in_link {
                    cursor_screen = Some((inner.x + visual_col + (cc - *bs) as u16, y));
                }
                visual_col += raw_w;
            } else {
                let display = link_display(&raw);
                buf.set_string(inner.x + visual_col, y, &display, link_style);
                visual_col += display.chars().count() as u16;
            }
            last_end = *be;
        }

        if cursor_on_this_line && cc >= last_end && cursor_screen.is_none() {
            cursor_screen = Some((inner.x + visual_col + (cc - last_end) as u16, y));
        }
        if last_end < chars.len() {
            let trailing: String = chars[last_end..].iter().collect();
            buf.set_string(inner.x + visual_col, y, &trailing, theme::text());
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

fn link_display(raw: &str) -> String {
    if raw.len() < 4 {
        return raw.to_string();
    }
    let inner = &raw[2..raw.len() - 2];
    let inner = inner.trim();
    if let Some(idx) = inner.find('|') {
        return inner[idx + 1..].trim().to_string();
    }
    let t = inner.split('#').next().unwrap_or(inner);
    t.trim().to_string()
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
            " up/down:links  enter:follow  ^l:make-link  ^p:cmds  tab:notes ",
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
    let block = Block::bordered()
        .border_style(theme::brand())
        .title(" graph — most connected (enter:open  esc:close) ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    let start = g.sel.saturating_sub(height.saturating_sub(1));
    let items: Vec<ListItem> = g
        .list
        .iter()
        .skip(start)
        .take(height)
        .enumerate()
        .map(|(vis, &ni)| {
            let abs = start + vis;
            let title = app
                .vault
                .notes
                .get(ni)
                .map(|n| n.title.as_str())
                .unwrap_or("?");
            let back = app.vault.backlinks(ni).len();
            let out = app.vault.outbound(ni).len();
            let style = if abs == g.sel {
                theme::selected()
            } else {
                theme::text()
            };
            let mut spans = vec![
                Span::styled(format!("{title}  "), style),
                Span::styled(format!("←{back} →{out}  "), theme::muted()),
            ];
            if back == 0 && out == 0 {
                spans.push(Span::styled("orphan", theme::faint()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    f.render_widget(List::new(items), inner);
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
    fn graph_view_opens_and_opens_note() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Hub.md"), "# Hub\n").unwrap();
        std::fs::write(dir.path().join("A.md"), "# A\n[[Hub]]\n").unwrap();
        let mut app = load(dir.path());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        assert!(app.graph.is_some());
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        assert!(buffer_text(terminal.backend().buffer()).contains("most connected"));

        app.on_key(key(KeyCode::Enter));
        assert!(app.graph.is_none());
        let idx = app.open.as_ref().unwrap().idx;
        assert_eq!(app.vault.notes[idx].title, "Hub");
    }
}
