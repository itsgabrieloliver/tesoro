use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, TextArea};

use crate::markdown::RenderedNote;
use crate::picker::Picker;
use crate::vault::Vault;

const SUPPRESS_WINDOW: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Reader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Edit,
    Preview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMode {
    Normal,
    Insert,
    Visual,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Delete,
    Yank,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    None,
    Op(Op),
    Inside(Op),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SwitcherMode {
    Notes,
    Tags,
    LinkOrCreate,
}

#[derive(Clone, Copy)]
pub enum LeaderKind {
    Modifier(KeyModifiers),
    Char(char),
}

pub fn parse_leader(s: &str) -> LeaderKind {
    let lower = s.trim().to_lowercase();
    match lower.as_str() {
        "ctrl" | "control" | "" => LeaderKind::Modifier(KeyModifiers::CONTROL),
        "alt" | "option" | "meta" => LeaderKind::Modifier(KeyModifiers::ALT),
        "space" | " " => LeaderKind::Char(' '),
        "\\" | "backslash" => LeaderKind::Char('\\'),
        "," | "comma" => LeaderKind::Char(','),
        _ if lower.chars().count() == 1 => LeaderKind::Char(lower.chars().next().unwrap()),
        _ => LeaderKind::Modifier(KeyModifiers::CONTROL),
    }
}

pub enum PromptKind {
    NewNote,
    AliasLink { row: usize, start: usize, end: usize },
    Rename { old_path: PathBuf },
}

pub struct Prompt {
    pub title: String,
    pub input: String,
    pub kind: PromptKind,
}

pub struct OpenNote {
    pub idx: usize,
    pub textarea: TextArea<'static>,
    pub view: ViewMode,
    pub mode: EditMode,
    pub dirty: bool,
    pub scroll: u16,
    pub width: u16,
    pub editor_top: u16,
    pub link_sel: usize,
    pub render: Option<Rc<RenderedNote>>,
}

pub struct SearchState {
    pub query: String,
    pub results: Vec<(usize, String)>,
    pub sel: usize,
}

pub struct GraphView {
    pub root_idx: usize,
    pub expanded: HashSet<Vec<usize>>,
    pub visible: Vec<TreeRow>,
    pub sel: usize,
}

#[derive(Clone, Copy)]
pub enum SlashKind {
    Link,
    Today,
    H1,
    H2,
    H3,
    Code,
    List,
    Checklist,
    Table,
    Quote,
    Divider,
}

#[derive(Clone)]
pub struct SlashItem {
    pub label: &'static str,
    pub description: &'static str,
    pub kind: SlashKind,
}

pub struct SlashMenu {
    pub filter: String,
    pub items: Vec<SlashItem>,
    pub sel: usize,
}

pub fn slash_items() -> Vec<SlashItem> {
    vec![
        SlashItem { label: "link", description: "wrap word in [[…]] or insert empty link", kind: SlashKind::Link },
        SlashItem { label: "today", description: "insert [[YYYY-MM-DD]] for today's daily", kind: SlashKind::Today },
        SlashItem { label: "h1", description: "insert # heading", kind: SlashKind::H1 },
        SlashItem { label: "h2", description: "insert ## heading", kind: SlashKind::H2 },
        SlashItem { label: "h3", description: "insert ### heading", kind: SlashKind::H3 },
        SlashItem { label: "code", description: "insert ``` fenced code block", kind: SlashKind::Code },
        SlashItem { label: "list", description: "insert - bullet", kind: SlashKind::List },
        SlashItem { label: "checklist", description: "insert - [ ] todo item", kind: SlashKind::Checklist },
        SlashItem { label: "table", description: "insert a 2x2 table skeleton", kind: SlashKind::Table },
        SlashItem { label: "quote", description: "insert > blockquote", kind: SlashKind::Quote },
        SlashItem { label: "divider", description: "insert --- horizontal rule", kind: SlashKind::Divider },
    ]
}

pub fn slash_filter(filter: &str) -> Vec<SlashItem> {
    let all = slash_items();
    if filter.is_empty() {
        return all;
    }
    let needle = filter.to_lowercase();
    all.into_iter()
        .filter(|it| it.label.to_lowercase().starts_with(&needle))
        .collect()
}

#[derive(Clone)]
pub struct TreeRow {
    pub note_idx: usize,
    pub path: Vec<usize>,
    pub depth: usize,
    pub has_children: bool,
    pub is_last_at_levels: Vec<bool>,
}

pub fn toggle_checkbox_line(line: &str) -> Option<String> {
    let lead: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let rest = &line[lead.len()..];
    let (bullet, after) = if let Some(s) = rest.strip_prefix("- ") {
        ("- ", s)
    } else if let Some(s) = rest.strip_prefix("* ") {
        ("* ", s)
    } else if let Some(s) = rest.strip_prefix("+ ") {
        ("+ ", s)
    } else {
        return None;
    };
    let tail = if let Some(after_box) = after.strip_prefix("[ ]") {
        format!("[x]{after_box}")
    } else if let Some(after_box) = after
        .strip_prefix("[x]")
        .or_else(|| after.strip_prefix("[X]"))
    {
        format!("[ ]{after_box}")
    } else {
        format!("[ ] {after}")
    };
    Some(format!("{lead}{bullet}{tail}"))
}

fn graph_build_visible(
    vault: &crate::vault::Vault,
    root_idx: usize,
    expanded: &HashSet<Vec<usize>>,
) -> Vec<TreeRow> {
    fn rec(
        vault: &crate::vault::Vault,
        note_idx: usize,
        path: Vec<usize>,
        depth: usize,
        levels: Vec<bool>,
        expanded: &HashSet<Vec<usize>>,
        rows: &mut Vec<TreeRow>,
    ) {
        let children: Vec<usize> = vault
            .outbound(note_idx)
            .into_iter()
            .filter(|c| !path.contains(c))
            .collect();
        let has_children = !children.is_empty();
        rows.push(TreeRow {
            note_idx,
            path: path.clone(),
            depth,
            has_children,
            is_last_at_levels: levels.clone(),
        });
        if expanded.contains(&path) {
            for (k, &child) in children.iter().enumerate() {
                let is_last = k + 1 == children.len();
                let mut new_levels = levels.clone();
                new_levels.push(is_last);
                let mut new_path = path.clone();
                new_path.push(child);
                rec(vault, child, new_path, depth + 1, new_levels, expanded, rows);
            }
        }
    }
    let mut rows = Vec::new();
    rec(vault, root_idx, vec![root_idx], 0, Vec::new(), expanded, &mut rows);
    rows
}

const COMMANDS: &[&str] = &[
    "quick switch",
    "search",
    "tags",
    "graph",
    "new note",
    "daily note",
    "alias link",
    "open in editor",
    "toggle panel",
    "toggle sidebar",
    "reload vault",
    "quit",
];

pub struct App {
    pub vault: Vault,
    pub focus: Focus,
    pub selected: usize,
    pub open: Option<OpenNote>,
    pub switcher: Option<Picker>,
    pub switcher_mode: SwitcherMode,
    pub search: Option<SearchState>,
    pub prompt: Option<Prompt>,
    pub palette: Option<Picker>,
    pub graph: Option<GraphView>,
    pub slash: Option<SlashMenu>,
    pub show_panel: bool,
    pub show_sidebar: bool,
    pub edit_request: Option<PathBuf>,
    pub status: Option<String>,
    pub should_quit: bool,
    pub tick: u64,
    pub pending: Pending,
    pub leader_kind: LeaderKind,
    pub leader_pending: bool,
    pub pinned: HashSet<PathBuf>,
    pub delete_pending: bool,
    pub format_on_save: bool,
    suppress: HashMap<PathBuf, Instant>,
}

fn load_pins(root: &Path) -> HashSet<PathBuf> {
    let path = root.join(".tesoro-pins.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    let list: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    list.into_iter().map(PathBuf::from).collect()
}

fn save_pins(root: &Path, pinned: &HashSet<PathBuf>) {
    let path = root.join(".tesoro-pins.json");
    let list: Vec<String> = pinned
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if let Ok(json) = serde_json::to_string(&list) {
        let _ = std::fs::write(path, json);
    }
}

fn clipboard_set(text: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(text.to_string());
    }
}

fn clipboard_get() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
}

impl App {
    pub fn new(vault: Vault) -> Self {
        let pinned = load_pins(&vault.root);
        Self {
            vault,
            focus: Focus::Sidebar,
            selected: 0,
            open: None,
            switcher: None,
            switcher_mode: SwitcherMode::Notes,
            search: None,
            prompt: None,
            palette: None,
            graph: None,
            slash: None,
            show_panel: false,
            show_sidebar: true,
            edit_request: None,
            status: None,
            should_quit: false,
            tick: 0,
            pending: Pending::None,
            leader_kind: LeaderKind::Modifier(KeyModifiers::CONTROL),
            leader_pending: false,
            pinned,
            delete_pending: false,
            format_on_save: true,
            suppress: HashMap::new(),
        }
    }

    pub fn generate_toc(&self) -> String {
        use std::collections::BTreeMap;
        let mut grouped: BTreeMap<String, Vec<&crate::vault::NoteMeta>> = BTreeMap::new();
        for note in &self.vault.notes {
            let parent = note
                .rel
                .parent()
                .map(|p| {
                    if p.as_os_str().is_empty() {
                        "/".to_string()
                    } else {
                        format!("/{}/", p.display())
                    }
                })
                .unwrap_or_else(|| "/".to_string());
            grouped.entry(parent).or_default().push(note);
        }
        let mut out = String::new();
        out.push_str("# Notes\n\n");
        if self.vault.notes.is_empty() {
            out.push_str("_Empty vault — press `n` in the sidebar to make a new note._\n");
            return out;
        }
        for (folder, mut notes) in grouped {
            out.push_str(&format!("## {folder}\n\n"));
            notes.sort_by(|a, b| a.title.cmp(&b.title));
            for n in notes {
                let pinned = self.pinned.contains(&n.path);
                let prefix = if pinned { "* " } else { "" };
                out.push_str(&format!("- {prefix}[[{}]]\n", n.title));
            }
            out.push('\n');
        }
        out
    }

    pub fn display_order(&self) -> Vec<usize> {
        let mut pinned: Vec<usize> = Vec::new();
        let mut other: Vec<usize> = Vec::new();
        for (i, n) in self.vault.notes.iter().enumerate() {
            if self.pinned.contains(&n.path) {
                pinned.push(i);
            } else {
                other.push(i);
            }
        }
        pinned.into_iter().chain(other).collect()
    }

    pub fn set_leader(&mut self, leader: LeaderKind) {
        self.leader_kind = leader;
        self.leader_pending = false;
    }

    pub fn open_welcome_in_preview(&mut self) {
        let Some(idx) = self.vault.resolve("Welcome") else {
            return;
        };
        let path = self.vault.notes[idx].path.clone();
        self.open_path(&path);
        if let Some(o) = self.open.as_mut() {
            o.view = ViewMode::Preview;
            o.render = None;
        }
        self.focus = Focus::Reader;
    }

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    fn in_edit(&self) -> bool {
        self.open
            .as_ref()
            .map(|o| matches!(o.view, ViewMode::Edit))
            .unwrap_or(false)
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if !self.show_sidebar && self.focus == Focus::Sidebar {
            self.focus = Focus::Reader;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit();
            return;
        }
        if self.switcher.is_some() {
            self.switcher_key(key);
            return;
        }
        if self.search.is_some() {
            self.search_key(key);
            return;
        }
        if self.prompt.is_some() {
            self.prompt_key(key);
            return;
        }
        if self.palette.is_some() {
            self.palette_key(key);
            return;
        }
        if self.graph.is_some() {
            self.graph_key(key);
            return;
        }

        match self.leader_kind {
            LeaderKind::Modifier(m) => {
                if key.modifiers == m && self.dispatch_leader(key.code) {
                    return;
                }
            }
            LeaderKind::Char(c) => {
                if self.leader_pending {
                    self.leader_pending = false;
                    self.dispatch_leader(key.code);
                    return;
                }
                if self.is_leader_context()
                    && key.code == KeyCode::Char(c)
                    && key.modifiers.is_empty()
                {
                    self.leader_pending = true;
                    return;
                }
            }
        }

        if self.focus == Focus::Reader && self.in_edit() {
            self.center_key(key);
            return;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => {
                self.quit();
                return;
            }
            KeyCode::Char('o') if ctrl => {
                self.open_switcher();
                return;
            }
            KeyCode::Char('p') if ctrl => {
                self.open_palette();
                return;
            }
            KeyCode::Char('b') if ctrl => {
                self.show_panel = !self.show_panel;
                return;
            }
            KeyCode::Char('w') if ctrl => {
                self.toggle_sidebar();
                return;
            }
            KeyCode::Char('/') => {
                self.open_search();
                return;
            }
            KeyCode::Char('t') => {
                self.open_tags();
                return;
            }
            KeyCode::Char('D') => {
                self.open_daily();
                return;
            }
            KeyCode::Tab => {
                self.toggle_focus();
                return;
            }
            _ => {}
        }
        match self.focus {
            Focus::Sidebar => self.sidebar_key(key),
            Focus::Reader => self.preview_key(key),
        }
    }

    fn toggle_focus(&mut self) {
        match self.focus {
            Focus::Sidebar => self.focus = Focus::Reader,
            Focus::Reader => {
                if self.show_sidebar {
                    self.focus = Focus::Sidebar;
                }
            }
        }
    }

    fn sidebar_key(&mut self, key: KeyEvent) {
        if self.delete_pending {
            self.delete_pending = false;
            if key.code == KeyCode::Char('d') {
                self.delete_selected();
            }
            return;
        }
        if key.code == KeyCode::Char('n') {
            self.prompt = Some(Prompt {
                title: "new note".to_string(),
                input: String::new(),
                kind: PromptKind::NewNote,
            });
            return;
        }
        let order = self.display_order();
        let n = order.len();
        if n == 0 {
            return;
        }
        let cur_pos = order.iter().position(|&i| i == self.selected).unwrap_or(0);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let new_pos = (cur_pos + 1).min(n - 1);
                self.selected = order[new_pos];
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let new_pos = cur_pos.saturating_sub(1);
                self.selected = order[new_pos];
            }
            KeyCode::Char('g') => self.selected = order[0],
            KeyCode::Char('G') => self.selected = order[n - 1],
            KeyCode::Enter | KeyCode::Char('l') => self.open_selected(),
            KeyCode::Char('d') => {
                self.delete_pending = true;
                if let Some(note) = self.vault.notes.get(self.selected) {
                    self.status = Some(format!("press d to delete '{}'", note.title));
                }
            }
            KeyCode::Char('r') => self.open_rename_prompt(),
            KeyCode::Char('p') => self.toggle_pin_selected(),
            _ => {}
        }
    }

    fn center_key(&mut self, key: KeyEvent) {
        match self.open.as_ref().map(|o| o.mode) {
            Some(EditMode::Insert) => self.insert_key(key),
            Some(EditMode::Normal) => self.normal_key(key),
            Some(EditMode::Visual) => self.visual_key(key),
            None => {}
        }
    }

    fn normal_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        match self.pending {
            Pending::None => {}
            Pending::Op(op) => {
                if key.code == KeyCode::Esc {
                    self.pending = Pending::None;
                    return;
                }
                self.pending = Pending::None;
                match (op, key.code) {
                    (Op::Delete, KeyCode::Char('d')) => return self.delete_line(),
                    (Op::Yank, KeyCode::Char('y')) => return self.yank_line(),
                    (Op::Delete, KeyCode::Char('w')) => return self.delete_word_forward(),
                    (Op::Yank, KeyCode::Char('w')) => return self.yank_word_forward(),
                    (op, KeyCode::Char('i')) => {
                        self.pending = Pending::Inside(op);
                        return;
                    }
                    _ => return,
                }
            }
            Pending::Inside(op) => {
                if key.code == KeyCode::Esc {
                    self.pending = Pending::None;
                    return;
                }
                self.pending = Pending::None;
                match (op, key.code) {
                    (Op::Delete, KeyCode::Char('w')) => return self.delete_inner_word(),
                    (Op::Yank, KeyCode::Char('w')) => return self.yank_inner_word(),
                    _ => return,
                }
            }
        }

        match key.code {
            KeyCode::Char('o') if ctrl => return self.open_switcher(),
            KeyCode::Char('p') if ctrl => return self.open_palette(),
            KeyCode::Char('b') if ctrl => {
                self.show_panel = !self.show_panel;
                return;
            }
            KeyCode::Char('w') if ctrl => {
                self.toggle_sidebar();
                return;
            }
            KeyCode::Char('s') if ctrl => return self.save_open(),
            KeyCode::Char('e') if ctrl => return self.set_view(ViewMode::Preview),
            KeyCode::Char('l') if ctrl => return self.make_link(),
            KeyCode::Char('l') if alt => return self.open_alias_prompt(),
            KeyCode::Char('r') if ctrl => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.redo();
                    o.dirty = true;
                }
                return;
            }
            KeyCode::Up => return self.jump_link(-1),
            KeyCode::Down => return self.jump_link(1),
            KeyCode::Enter => return self.follow_link_under_cursor(),
            KeyCode::Tab | KeyCode::Esc => {
                if self.show_sidebar {
                    self.focus = Focus::Sidebar;
                }
                return;
            }
            KeyCode::Char('d') => {
                self.pending = Pending::Op(Op::Delete);
                return;
            }
            KeyCode::Char('y') => {
                self.pending = Pending::Op(Op::Yank);
                return;
            }
            KeyCode::Char('v') => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.start_selection();
                    o.mode = EditMode::Visual;
                }
                return;
            }
            KeyCode::Char('p') => return self.paste(false),
            KeyCode::Char('P') => return self.paste(true),
            _ => {}
        }
        let Some(o) = self.open.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => o.textarea.move_cursor(CursorMove::Back),
            KeyCode::Char('l') | KeyCode::Right => o.textarea.move_cursor(CursorMove::Forward),
            KeyCode::Char('j') => o.textarea.move_cursor(CursorMove::Down),
            KeyCode::Char('k') => o.textarea.move_cursor(CursorMove::Up),
            KeyCode::Char('w') => o.textarea.move_cursor(CursorMove::WordForward),
            KeyCode::Char('e') => o.textarea.move_cursor(CursorMove::WordEnd),
            KeyCode::Char('b') => o.textarea.move_cursor(CursorMove::WordBack),
            KeyCode::Char('0') => o.textarea.move_cursor(CursorMove::Head),
            KeyCode::Char('$') => o.textarea.move_cursor(CursorMove::End),
            KeyCode::Char('g') => o.textarea.move_cursor(CursorMove::Top),
            KeyCode::Char('G') => o.textarea.move_cursor(CursorMove::Bottom),
            KeyCode::Char('i') => o.mode = EditMode::Insert,
            KeyCode::Char('a') => {
                o.textarea.move_cursor(CursorMove::Forward);
                o.mode = EditMode::Insert;
            }
            KeyCode::Char('A') => {
                o.textarea.move_cursor(CursorMove::End);
                o.mode = EditMode::Insert;
            }
            KeyCode::Char('o') => {
                o.textarea.move_cursor(CursorMove::End);
                o.textarea.insert_newline();
                o.mode = EditMode::Insert;
                o.dirty = true;
            }
            KeyCode::Char('x') => {
                o.textarea.delete_next_char();
                o.dirty = true;
            }
            KeyCode::Char('D') => {
                o.textarea.delete_line_by_end();
                o.dirty = true;
            }
            KeyCode::Char('u') => {
                o.textarea.undo();
                o.dirty = true;
            }
            _ => {}
        }
    }

    fn insert_key(&mut self, key: KeyEvent) {
        if self.slash.is_some() {
            return self.slash_key(key);
        }
        if key.code == KeyCode::Esc {
            if let Some(o) = self.open.as_mut() {
                o.mode = EditMode::Normal;
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.save_open();
            return;
        }
        if key.code == KeyCode::Char('/')
            && key.modifiers.is_empty()
            && self.cursor_after_break()
        {
            self.slash = Some(SlashMenu {
                filter: String::new(),
                items: slash_items(),
                sel: 0,
            });
        }
        if let Some(o) = self.open.as_mut()
            && o.textarea.input(key)
        {
            o.dirty = true;
        }
    }

    fn cursor_after_break(&self) -> bool {
        let Some(o) = self.open.as_ref() else { return false };
        let (row, col) = o.textarea.cursor();
        if col == 0 {
            return true;
        }
        let lines = o.textarea.lines();
        let chars: Vec<char> = lines
            .get(row)
            .map(|l| l.chars().collect())
            .unwrap_or_default();
        match chars.get(col - 1) {
            Some(c) => c.is_whitespace(),
            None => true,
        }
    }

    fn slash_key(&mut self, key: KeyEvent) {
        let Some(menu) = self.slash.as_mut() else { return };
        match key.code {
            KeyCode::Esc => {
                self.slash = None;
                return;
            }
            KeyCode::Up => {
                if menu.sel > 0 {
                    menu.sel -= 1;
                }
                return;
            }
            KeyCode::Down => {
                if menu.sel + 1 < menu.items.len() {
                    menu.sel += 1;
                }
                return;
            }
            KeyCode::Tab => {
                if !menu.items.is_empty() {
                    menu.sel = (menu.sel + 1) % menu.items.len();
                }
                return;
            }
            KeyCode::Enter => {
                let kind = menu.items.get(menu.sel).map(|it| it.kind);
                let filter_chars = menu.filter.chars().count();
                self.slash = None;
                self.delete_slash_text(filter_chars);
                if let Some(k) = kind {
                    self.execute_slash(k);
                }
                return;
            }
            KeyCode::Backspace => {
                if menu.filter.is_empty() {
                    self.slash = None;
                    if let Some(o) = self.open.as_mut() {
                        o.textarea.delete_char();
                        o.dirty = true;
                    }
                    return;
                }
                menu.filter.pop();
                menu.items = slash_filter(&menu.filter);
                if menu.items.is_empty() {
                    menu.sel = 0;
                } else if menu.sel >= menu.items.len() {
                    menu.sel = menu.items.len() - 1;
                }
                if let Some(o) = self.open.as_mut() {
                    o.textarea.delete_char();
                    o.dirty = true;
                }
                return;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                menu.filter.push(c);
                menu.items = slash_filter(&menu.filter);
                if menu.items.is_empty() {
                    menu.sel = 0;
                } else if menu.sel >= menu.items.len() {
                    menu.sel = menu.items.len() - 1;
                }
                if let Some(o) = self.open.as_mut()
                    && o.textarea.input(key)
                {
                    o.dirty = true;
                }
                return;
            }
            _ => {}
        }
    }

    fn delete_slash_text(&mut self, filter_chars: usize) {
        let Some(o) = self.open.as_mut() else { return };
        for _ in 0..(filter_chars + 1) {
            o.textarea.delete_char();
        }
        o.dirty = true;
    }

    fn execute_slash(&mut self, kind: SlashKind) {
        match kind {
            SlashKind::Link => self.make_link(),
            SlashKind::Today => {
                let stamp = chrono::Local::now().format("%Y-%m-%d").to_string();
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str(format!("[[{stamp}]]"));
                    o.dirty = true;
                }
            }
            SlashKind::H1 => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("# ");
                    o.dirty = true;
                }
            }
            SlashKind::H2 => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("## ");
                    o.dirty = true;
                }
            }
            SlashKind::H3 => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("### ");
                    o.dirty = true;
                }
            }
            SlashKind::Code => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("```\n\n```");
                    o.textarea.move_cursor(CursorMove::Up);
                    o.dirty = true;
                }
            }
            SlashKind::List => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("- ");
                    o.dirty = true;
                }
            }
            SlashKind::Checklist => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("- [ ] ");
                    o.dirty = true;
                }
            }
            SlashKind::Table => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea
                        .insert_str("| col 1 | col 2 |\n| ----- | ----- |\n|       |       |\n");
                    o.dirty = true;
                }
            }
            SlashKind::Quote => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("> ");
                    o.dirty = true;
                }
            }
            SlashKind::Divider => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.insert_str("\n---\n");
                    o.dirty = true;
                }
            }
        }
    }

    fn visual_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc | KeyCode::Char('v') => {
                if let Some(o) = self.open.as_mut() {
                    o.textarea.cancel_selection();
                    o.mode = EditMode::Normal;
                }
                return;
            }
            KeyCode::Char('y') => return self.visual_yank(),
            KeyCode::Char('d') | KeyCode::Char('x') => return self.visual_delete(),
            KeyCode::Char('Y') => return self.visual_yank_line(),
            KeyCode::Char('D') => return self.visual_delete_line(),
            KeyCode::Char('l') if ctrl => return self.visual_make_link(),
            KeyCode::Char('l') if alt => return self.visual_alias_prompt(),
            _ => {}
        }
        let Some(o) = self.open.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => o.textarea.move_cursor(CursorMove::Back),
            KeyCode::Char('l') | KeyCode::Right => o.textarea.move_cursor(CursorMove::Forward),
            KeyCode::Char('j') | KeyCode::Down => o.textarea.move_cursor(CursorMove::Down),
            KeyCode::Char('k') | KeyCode::Up => o.textarea.move_cursor(CursorMove::Up),
            KeyCode::Char('w') => o.textarea.move_cursor(CursorMove::WordForward),
            KeyCode::Char('e') => o.textarea.move_cursor(CursorMove::WordEnd),
            KeyCode::Char('b') => o.textarea.move_cursor(CursorMove::WordBack),
            KeyCode::Char('0') => o.textarea.move_cursor(CursorMove::Head),
            KeyCode::Char('$') => o.textarea.move_cursor(CursorMove::End),
            KeyCode::Char('g') => o.textarea.move_cursor(CursorMove::Top),
            KeyCode::Char('G') => o.textarea.move_cursor(CursorMove::Bottom),
            _ => {}
        }
    }

    fn visual_yank(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.textarea.copy();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
        }
        o.textarea.cancel_selection();
        o.mode = EditMode::Normal;
    }

    fn visual_delete(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.textarea.cut();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
            o.dirty = true;
        }
        o.textarea.cancel_selection();
        o.mode = EditMode::Normal;
    }

    fn visual_make_link(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.textarea.copy();
        let content = o.textarea.yank_text();
        if content.is_empty() {
            return;
        }
        o.textarea.cut();
        o.textarea.insert_str(format!("[[{content}]]"));
        o.textarea.cancel_selection();
        o.mode = EditMode::Normal;
        o.dirty = true;
    }

    fn visual_alias_prompt(&mut self) {
        let (content, row, col) = {
            let Some(o) = self.open.as_mut() else {
                return;
            };
            o.textarea.copy();
            let c = o.textarea.yank_text();
            if c.is_empty() {
                return;
            }
            o.textarea.cut();
            o.mode = EditMode::Normal;
            o.dirty = true;
            let (r, k) = o.textarea.cursor();
            (c, r, k)
        };
        let prefill = if content.is_empty() {
            String::new()
        } else {
            format!("{content}|{content}")
        };
        self.prompt = Some(Prompt {
            title: "link (target|display)".to_string(),
            input: prefill,
            kind: PromptKind::AliasLink {
                row,
                start: col,
                end: col,
            },
        });
    }

    fn paste(&mut self, before: bool) {
        let cb_text = clipboard_get();
        let Some(o) = self.open.as_mut() else {
            return;
        };
        if let Some(t) = cb_text {
            o.textarea.set_yank_text(t);
        }
        if !before {
            o.textarea.move_cursor(CursorMove::Forward);
        }
        o.textarea.paste();
        o.dirty = true;
    }

    fn delete_line(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let n_lines = o.textarea.lines().len();
        o.textarea.move_cursor(CursorMove::Head);
        o.textarea.start_selection();
        let (cr, _) = o.textarea.cursor();
        if cr + 1 < n_lines {
            o.textarea.move_cursor(CursorMove::Down);
            o.textarea.move_cursor(CursorMove::Head);
        } else {
            o.textarea.move_cursor(CursorMove::End);
        }
        o.textarea.cut();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
            o.dirty = true;
        }
        o.textarea.cancel_selection();
    }

    fn yank_line(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let n_lines = o.textarea.lines().len();
        let (sr, sc) = o.textarea.cursor();
        o.textarea.move_cursor(CursorMove::Head);
        o.textarea.start_selection();
        let (cr, _) = o.textarea.cursor();
        if cr + 1 < n_lines {
            o.textarea.move_cursor(CursorMove::Down);
            o.textarea.move_cursor(CursorMove::Head);
        } else {
            o.textarea.move_cursor(CursorMove::End);
        }
        o.textarea.copy();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
        }
        o.textarea.cancel_selection();
        o.textarea
            .move_cursor(CursorMove::Jump(sr as u16, sc as u16));
    }

    fn delete_word_forward(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.textarea.start_selection();
        o.textarea.move_cursor(CursorMove::WordForward);
        o.textarea.cut();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
            o.dirty = true;
        }
        o.textarea.cancel_selection();
    }

    fn yank_word_forward(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let (sr, sc) = o.textarea.cursor();
        o.textarea.start_selection();
        o.textarea.move_cursor(CursorMove::WordForward);
        o.textarea.copy();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
        }
        o.textarea.cancel_selection();
        o.textarea
            .move_cursor(CursorMove::Jump(sr as u16, sc as u16));
    }

    fn select_inner_word(&mut self) -> bool {
        let Some(o) = self.open.as_mut() else {
            return false;
        };
        let (cr, cc) = o.textarea.cursor();
        let line = o.textarea.lines().get(cr).cloned().unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut start = cc.min(chars.len());
        let mut end = cc.min(chars.len());
        while start > 0 && is_word(chars[start - 1]) {
            start -= 1;
        }
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        if start == end {
            return false;
        }
        o.textarea
            .move_cursor(CursorMove::Jump(cr as u16, start as u16));
        o.textarea.start_selection();
        o.textarea
            .move_cursor(CursorMove::Jump(cr as u16, end as u16));
        true
    }

    fn delete_inner_word(&mut self) {
        if !self.select_inner_word() {
            return;
        }
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.textarea.cut();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
            o.dirty = true;
        }
        o.textarea.cancel_selection();
    }

    fn yank_inner_word(&mut self) {
        if !self.select_inner_word() {
            return;
        }
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.textarea.copy();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
        }
        o.textarea.cancel_selection();
    }

    fn preview_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.set_view(ViewMode::Edit);
            }
            KeyCode::Char('i') => return self.set_view(ViewMode::Edit),
            KeyCode::Char('h') | KeyCode::Esc => {
                if self.show_sidebar {
                    self.focus = Focus::Sidebar;
                }
                return;
            }
            KeyCode::Char('n') | KeyCode::Down => return self.cycle_link(1),
            KeyCode::Char('N') | KeyCode::Up => return self.cycle_link(-1),
            KeyCode::Enter => return self.follow_selected_link(),
            _ => {}
        }
        let max = self
            .open
            .as_ref()
            .and_then(|o| o.render.as_ref())
            .map(|r| r.lines.len() as u16)
            .unwrap_or(0);
        let last = max.saturating_sub(1);
        let Some(o) = self.open.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Char('j') => o.scroll = (o.scroll + 1).min(last),
            KeyCode::Char('k') => o.scroll = o.scroll.saturating_sub(1),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                o.scroll = (o.scroll + 10).min(last);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                o.scroll = o.scroll.saturating_sub(10);
            }
            KeyCode::Char('g') => o.scroll = 0,
            KeyCode::Char('G') => o.scroll = last,
            _ => {}
        }
    }

    fn set_view(&mut self, view: ViewMode) {
        if let Some(o) = self.open.as_mut() {
            match view {
                ViewMode::Preview => {
                    o.view = ViewMode::Preview;
                    o.render = None;
                }
                ViewMode::Edit => {
                    o.view = ViewMode::Edit;
                    o.mode = EditMode::Normal;
                }
            }
        }
    }

    fn jump_link(&mut self, dir: isize) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let positions: Vec<(usize, usize)> = {
            let mut v = Vec::new();
            for (row, line) in o.textarea.lines().iter().enumerate() {
                for (cs, _, _) in crate::markdown::wikilink_positions(line) {
                    v.push((row, cs));
                }
            }
            v
        };
        if positions.is_empty() {
            return;
        }
        let (cr, cc) = o.textarea.cursor();
        let target = if dir > 0 {
            positions
                .iter()
                .find(|&&(r, c)| r > cr || (r == cr && c > cc))
                .copied()
                .or_else(|| positions.first().copied())
        } else {
            positions
                .iter()
                .rev()
                .find(|&&(r, c)| r < cr || (r == cr && c < cc))
                .copied()
                .or_else(|| positions.last().copied())
        };
        if let Some((r, c)) = target {
            o.textarea.move_cursor(CursorMove::Jump(r as u16, c as u16));
        }
    }

    fn follow_link_under_cursor(&mut self) {
        let target = self.open.as_ref().and_then(|o| {
            let (cr, cc) = o.textarea.cursor();
            let line = o.textarea.lines().get(cr)?.clone();
            let links = crate::markdown::wikilink_positions(&line);
            links
                .iter()
                .find(|(cs, ce, _)| cc >= *cs && cc < *ce)
                .map(|(_, _, t)| t.clone())
                .or_else(|| {
                    links
                        .iter()
                        .find(|(cs, _, _)| *cs >= cc)
                        .map(|(_, _, t)| t.clone())
                })
        });
        if let Some(target) = target {
            self.open_link_target(&target);
        }
    }

    fn make_link(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let (cr, cc) = o.textarea.cursor();
        let line = o.textarea.lines().get(cr).cloned().unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
        let mut start = cc.min(chars.len());
        let mut end = cc.min(chars.len());
        while start > 0 && is_word(chars[start - 1]) {
            start -= 1;
        }
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        if start < end {
            let word: String = chars[start..end].iter().collect();
            let prefix: String = chars[..start].iter().collect();
            let suffix: String = chars[end..].iter().collect();
            let new_line = format!("{prefix}[[{word}]]{suffix}");
            o.textarea.move_cursor(CursorMove::Jump(cr as u16, 0));
            o.textarea.delete_line_by_end();
            o.textarea.insert_str(new_line);
        } else {
            o.textarea.insert_str("[[]]");
            o.textarea.move_cursor(CursorMove::Back);
            o.textarea.move_cursor(CursorMove::Back);
        }
        o.dirty = true;
    }

    fn open_alias_prompt(&mut self) {
        let (row, start, end, word) = {
            let Some(o) = self.open.as_ref() else {
                return;
            };
            let (cr, cc) = o.textarea.cursor();
            let line = o.textarea.lines().get(cr).cloned().unwrap_or_default();
            let chars: Vec<char> = line.chars().collect();
            let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
            let mut s = cc.min(chars.len());
            let mut e = cc.min(chars.len());
            while s > 0 && is_word(chars[s - 1]) {
                s -= 1;
            }
            while e < chars.len() && is_word(chars[e]) {
                e += 1;
            }
            let word: String = chars[s..e].iter().collect();
            (cr, s, e, word)
        };
        let prefill = if word.is_empty() {
            String::new()
        } else {
            format!("{word}|{word}")
        };
        self.prompt = Some(Prompt {
            title: "link (target|display)".to_string(),
            input: prefill,
            kind: PromptKind::AliasLink { row, start, end },
        });
    }

    fn wrap_aliased_link(&mut self, input: &str, row: usize, start: usize, end: usize) {
        let input = input.trim();
        if input.is_empty() {
            return;
        }
        let link = format!("[[{input}]]");
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let line = o.textarea.lines().get(row).cloned().unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let prefix: String = chars[..start.min(chars.len())].iter().collect();
        let suffix: String = chars[end.min(chars.len())..].iter().collect();
        let new_line = format!("{prefix}{link}{suffix}");
        o.textarea.move_cursor(CursorMove::Jump(row as u16, 0));
        o.textarea.delete_line_by_end();
        o.textarea.insert_str(new_line);
        o.dirty = true;
    }

    fn switcher_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.switcher = None,
            KeyCode::Enter => self.switcher_accept(),
            KeyCode::Backspace => {
                if let Some(p) = self.switcher.as_mut() {
                    p.backspace();
                }
                if self.switcher_mode == SwitcherMode::LinkOrCreate {
                    self.relink_picker();
                }
            }
            KeyCode::Up => {
                if let Some(p) = self.switcher.as_mut() {
                    p.up();
                }
            }
            KeyCode::Down => {
                if let Some(p) = self.switcher.as_mut() {
                    p.down();
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(p) = self.switcher.as_mut() {
                    p.down();
                }
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(p) = self.switcher.as_mut() {
                    p.up();
                }
            }
            KeyCode::Char(c) => {
                if let Some(p) = self.switcher.as_mut() {
                    p.push(c);
                }
                if self.switcher_mode == SwitcherMode::LinkOrCreate {
                    self.relink_picker();
                }
            }
            _ => {}
        }
    }

    fn switcher_accept(&mut self) {
        match self.switcher_mode {
            SwitcherMode::Tags => {
                let tag = self
                    .switcher
                    .as_ref()
                    .and_then(|p| p.selected().map(|i| p.items[i].clone()));
                if let Some(tag) = tag {
                    let ids: Vec<usize> = self.vault.notes_with_tag(&tag).to_vec();
                    let items: Vec<String> = ids
                        .iter()
                        .filter_map(|&i| self.vault.notes.get(i))
                        .map(|n| n.title.clone())
                        .collect();
                    self.switcher = Some(Picker::with_map(items, ids));
                    self.switcher_mode = SwitcherMode::Notes;
                }
            }
            SwitcherMode::LinkOrCreate => {
                let (matches_empty, query, id) = match self.switcher.as_ref() {
                    Some(p) => (p.matches.is_empty(), p.query.clone(), p.selected_id()),
                    None => return,
                };
                self.switcher = None;
                if matches_empty {
                    if !query.trim().is_empty() {
                        self.new_note(&query);
                    }
                } else if let Some(i) = id {
                    let notes_len = self.vault.notes.len();
                    if i >= notes_len {
                        let slash_idx = i - notes_len;
                        if let Some(item) = slash_items().get(slash_idx) {
                            self.execute_slash(item.kind);
                        }
                    } else {
                        let path = self.vault.notes.get(i).map(|n| n.path.clone());
                        if let Some(path) = path {
                            self.open_path(&path);
                        }
                    }
                }
            }
            SwitcherMode::Notes => {
                let id = self.switcher.as_ref().and_then(|p| p.selected_id());
                self.switcher = None;
                let path = id
                    .and_then(|i| self.vault.notes.get(i))
                    .map(|n| n.path.clone());
                if let Some(path) = path {
                    self.open_path(&path);
                }
            }
        }
    }

    fn open_link_or_create_search(&mut self) {
        self.switcher = Some(self.build_link_picker(""));
        self.switcher_mode = SwitcherMode::LinkOrCreate;
    }

    fn build_link_picker(&self, query: &str) -> Picker {
        let mut items: Vec<String> = Vec::new();
        let mut map: Vec<usize> = Vec::new();
        for (i, n) in self.vault.notes.iter().enumerate() {
            items.push(n.title.clone());
            map.push(i);
            for a in &n.aliases {
                items.push(a.clone());
                map.push(i);
            }
        }
        if !query.trim().is_empty() {
            for (idx, snippet) in self.vault.search(query) {
                let title = self
                    .vault
                    .notes
                    .get(idx)
                    .map(|n| n.title.as_str())
                    .unwrap_or("?");
                items.push(format!("{title}: {snippet}"));
                map.push(idx);
            }
        }
        let notes_len = self.vault.notes.len();
        for (k, it) in slash_items().iter().enumerate() {
            items.push(format!("/{}  {}", it.label, it.description));
            map.push(notes_len + k);
        }
        let mut p = Picker::with_map(items, map);
        if !query.is_empty() {
            for ch in query.chars() {
                p.query.push(ch);
            }
            p.refilter();
        }
        p
    }

    fn relink_picker(&mut self) {
        let query = match self.switcher.as_ref() {
            Some(p) => p.query.clone(),
            None => return,
        };
        self.switcher = Some(self.build_link_picker(&query));
    }

    fn open_search(&mut self) {
        self.search = Some(SearchState {
            query: String::new(),
            results: Vec::new(),
            sel: 0,
        });
    }

    fn search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.search = None,
            KeyCode::Enter => {
                let idx = self
                    .search
                    .as_ref()
                    .and_then(|s| s.results.get(s.sel).map(|(i, _)| *i));
                self.search = None;
                let path = idx
                    .and_then(|i| self.vault.notes.get(i))
                    .map(|n| n.path.clone());
                if let Some(path) = path {
                    self.open_path(&path);
                }
            }
            KeyCode::Up => self.search_move(-1),
            KeyCode::Down => self.search_move(1),
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_move(-1)
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_move(1)
            }
            KeyCode::Backspace => self.search_edit(|q| {
                q.pop();
            }),
            KeyCode::Char(c) => self.search_edit(|q| q.push(c)),
            _ => {}
        }
    }

    fn search_move(&mut self, delta: isize) {
        if let Some(s) = self.search.as_mut() {
            if s.results.is_empty() {
                s.sel = 0;
                return;
            }
            let cur = s.sel as isize;
            s.sel = (cur + delta).clamp(0, s.results.len() as isize - 1) as usize;
        }
    }

    fn search_edit(&mut self, f: impl FnOnce(&mut String)) {
        if self.search.is_none() {
            return;
        }
        {
            let s = self.search.as_mut().unwrap();
            f(&mut s.query);
            s.sel = 0;
        }
        let query = self.search.as_ref().unwrap().query.clone();
        let results = self.vault.search(&query);
        if let Some(s) = self.search.as_mut() {
            s.results = results;
        }
    }

    fn prompt_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.prompt = None,
            KeyCode::Enter => {
                let Some(p) = self.prompt.take() else {
                    return;
                };
                match p.kind {
                    PromptKind::NewNote => self.new_note(&p.input),
                    PromptKind::AliasLink { row, start, end } => {
                        self.wrap_aliased_link(&p.input, row, start, end)
                    }
                    PromptKind::Rename { old_path } => self.rename_to(&old_path, &p.input),
                }
            }
            KeyCode::Backspace => {
                if let Some(p) = self.prompt.as_mut() {
                    p.input.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(p) = self.prompt.as_mut() {
                    p.input.push(c);
                }
            }
            _ => {}
        }
    }

    fn open_palette(&mut self) {
        let items = COMMANDS.iter().map(|s| s.to_string()).collect();
        self.palette = Some(Picker::new(items));
    }

    fn palette_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.palette = None,
            KeyCode::Enter => {
                let cmd = self
                    .palette
                    .as_ref()
                    .and_then(|p| p.selected().map(|i| p.items[i].clone()));
                self.palette = None;
                if let Some(cmd) = cmd {
                    self.run_command(&cmd);
                }
            }
            KeyCode::Backspace => {
                if let Some(p) = self.palette.as_mut() {
                    p.backspace();
                }
            }
            KeyCode::Up => {
                if let Some(p) = self.palette.as_mut() {
                    p.up();
                }
            }
            KeyCode::Down => {
                if let Some(p) = self.palette.as_mut() {
                    p.down();
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(p) = self.palette.as_mut() {
                    p.down();
                }
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(p) = self.palette.as_mut() {
                    p.up();
                }
            }
            KeyCode::Char(c) => {
                if let Some(p) = self.palette.as_mut() {
                    p.push(c);
                }
            }
            _ => {}
        }
    }

    fn run_command(&mut self, cmd: &str) {
        match cmd {
            "quick switch" => self.open_switcher(),
            "search" => self.open_search(),
            "tags" => self.open_tags(),
            "graph" => self.open_graph(),
            "new note" => {
                self.prompt = Some(Prompt {
                    title: "new note".to_string(),
                    input: String::new(),
                    kind: PromptKind::NewNote,
                })
            }
            "daily note" => self.open_daily(),
            "alias link" => self.open_alias_prompt(),
            "open in editor" => self.request_edit(),
            "toggle panel" => self.show_panel = !self.show_panel,
            "toggle sidebar" => self.toggle_sidebar(),
            "reload vault" => self.reload_vault(),
            "quit" => self.quit(),
            _ => {}
        }
    }

    fn open_graph(&mut self) {
        let root_idx = self
            .open
            .as_ref()
            .map(|o| o.idx)
            .or_else(|| self.vault.most_connected().first().copied())
            .unwrap_or(0);
        if self.vault.notes.is_empty() {
            return;
        }
        let mut expanded: HashSet<Vec<usize>> = HashSet::new();
        expanded.insert(vec![root_idx]);
        let visible = graph_build_visible(&self.vault, root_idx, &expanded);
        self.graph = Some(GraphView {
            root_idx,
            expanded,
            visible,
            sel: 0,
        });
    }

    fn graph_key(&mut self, key: KeyEvent) {
        let Some(g) = self.graph.as_mut() else { return };
        match key.code {
            KeyCode::Esc => self.graph = None,
            KeyCode::Up | KeyCode::Char('k') => {
                if g.sel > 0 {
                    g.sel -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if g.sel + 1 < g.visible.len() {
                    g.sel += 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let row = g.visible.get(g.sel).cloned();
                if let Some(row) = row {
                    if row.has_children && !g.expanded.contains(&row.path) {
                        g.expanded.insert(row.path);
                        g.visible = graph_build_visible(&self.vault, g.root_idx, &g.expanded);
                    } else if g.sel + 1 < g.visible.len() {
                        g.sel += 1;
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let row = g.visible.get(g.sel).cloned();
                if let Some(row) = row {
                    if g.expanded.contains(&row.path) {
                        g.expanded.remove(&row.path);
                        g.visible = graph_build_visible(&self.vault, g.root_idx, &g.expanded);
                    } else if row.depth > 0 {
                        let parent_path = row.path[..row.path.len() - 1].to_vec();
                        if let Some(idx) = g
                            .visible
                            .iter()
                            .position(|r| r.path == parent_path)
                        {
                            g.sel = idx;
                        }
                    }
                }
            }
            KeyCode::Enter => {
                let target = g.visible.get(g.sel).map(|r| r.note_idx);
                self.graph = None;
                if let Some(i) = target {
                    if let Some(note) = self.vault.notes.get(i) {
                        let path = note.path.clone();
                        self.open_path(&path);
                    }
                }
            }
            _ => {}
        }
    }

    fn reload_vault(&mut self) {
        if let Ok(v) = Vault::load(self.vault.root.clone()) {
            self.vault = v;
            self.open = None;
            self.selected = 0;
            self.status = Some("vault reloaded".to_string());
        }
    }

    fn open_tags(&mut self) {
        let items: Vec<String> = self.vault.tags().into_iter().map(|(t, _)| t).collect();
        self.switcher = Some(Picker::new(items));
        self.switcher_mode = SwitcherMode::Tags;
    }

    fn open_switcher(&mut self) {
        let items = self
            .vault
            .notes
            .iter()
            .map(|n| {
                let rel = n.rel.to_string_lossy();
                rel.strip_suffix(".md").unwrap_or(&rel).to_string()
            })
            .collect();
        self.switcher = Some(Picker::new(items));
        self.switcher_mode = SwitcherMode::Notes;
    }

    fn open_selected(&mut self) {
        let Some(note) = self.vault.notes.get(self.selected) else {
            return;
        };
        let path = note.path.clone();
        self.open_path(&path);
    }

    fn open_path(&mut self, path: &Path) {
        self.save_open();
        let Some(idx) = self.vault.index_of(path) else {
            return;
        };
        let Ok(src) = std::fs::read_to_string(path) else {
            return;
        };
        let mut lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        textarea.set_selection_style(
            ratatui::style::Style::default()
                .bg(ratatui::style::Color::Indexed(238))
                .fg(ratatui::style::Color::Indexed(255)),
        );
        self.selected = idx;
        self.open = Some(OpenNote {
            idx,
            textarea,
            view: ViewMode::Edit,
            mode: EditMode::Normal,
            dirty: false,
            scroll: 0,
            width: 0,
            editor_top: 0,
            link_sel: 0,
            render: None,
        });
        self.focus = Focus::Reader;
        self.pending = Pending::None;
    }

    fn open_link_target(&mut self, target: &str) {
        if let Some(idx) = self.vault.resolve(target) {
            let path = self.vault.notes[idx].path.clone();
            self.open_path(&path);
            return;
        }
        let title = target.trim().to_string();
        let path = self.vault.root.join(format!("{title}.md"));
        if self.write_note(&path, &title, "note") {
            self.suppress(&path);
            self.vault.reload(&path);
            self.open_path(&path);
            self.status = Some(format!("created {title}"));
        }
    }

    fn cycle_link(&mut self, delta: isize) {
        let n = self
            .open
            .as_ref()
            .and_then(|o| o.render.as_ref())
            .map(|r| r.links.len())
            .unwrap_or(0);
        if n == 0 {
            return;
        }
        if let Some(o) = self.open.as_mut() {
            let cur = o.link_sel as isize;
            o.link_sel = (cur + delta).rem_euclid(n as isize) as usize;
        }
        let row = self.open.as_ref().and_then(|o| {
            o.render
                .as_ref()
                .and_then(|r| r.links.get(o.link_sel))
                .map(|l| l.row)
        });
        if let Some(row) = row
            && let Some(o) = self.open.as_mut()
        {
            o.scroll = (row as u16).saturating_sub(2);
        }
    }

    fn follow_selected_link(&mut self) {
        let target = self.open.as_ref().and_then(|o| {
            o.render
                .as_ref()
                .and_then(|r| r.links.get(o.link_sel))
                .map(|l| l.target.clone())
        });
        if let Some(target) = target {
            self.open_link_target(&target);
        }
    }

    fn new_note(&mut self, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        let path = self.vault.root.join(format!("{title}.md"));
        if self.write_note(&path, title, "note") {
            self.suppress(&path);
            self.vault.reload(&path);
            self.open_path(&path);
            self.status = Some(format!("created {title}"));
        }
    }

    fn open_daily(&mut self) {
        let path = self.vault.root.join(crate::templates::daily_filename());
        let title = crate::templates::today();
        if self.write_note(&path, &title, "daily") {
            self.suppress(&path);
            self.vault.reload(&path);
            self.open_path(&path);
            self.status = Some(format!("daily {title}"));
        }
    }

    fn write_note(&self, path: &Path, title: &str, template: &str) -> bool {
        if path.exists() {
            return true;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let body = crate::templates::load(&self.vault.root, template)
            .map(|t| crate::templates::render(&t, title))
            .unwrap_or_else(|| format!("# {title}\n"));
        std::fs::write(path, body).is_ok()
    }

    fn request_edit(&mut self) {
        let idx = if self.focus == Focus::Reader {
            self.open.as_ref().map(|o| o.idx)
        } else {
            Some(self.selected)
        };
        if let Some(i) = idx
            && let Some(note) = self.vault.notes.get(i)
        {
            self.edit_request = Some(note.path.clone());
        }
    }

    pub fn save_open(&mut self) {
        let Some(o) = self.open.as_ref() else {
            return;
        };
        if !o.dirty {
            return;
        }
        let Some(note) = self.vault.notes.get(o.idx) else {
            return;
        };
        let path = note.path.clone();
        let mut text = o.textarea.lines().join("\n");
        if !text.ends_with('\n') {
            text.push('\n');
        }
        if self.format_on_save {
            text = crate::format::format_markdown(&text);
        }
        self.suppress(&path);
        let _ = std::fs::write(&path, &text);
        self.vault.reload(&path);
        if let Some(o) = self.open.as_mut() {
            if self.format_on_save {
                let (cr, cc) = o.textarea.cursor();
                let lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
                let lines = if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
                    lines[..lines.len() - 1].to_vec()
                } else {
                    lines
                };
                let mut ta = tui_textarea::TextArea::new(if lines.is_empty() { vec![String::new()] } else { lines });
                ta.set_cursor_line_style(ratatui::style::Style::default());
                ta.set_selection_style(
                    ratatui::style::Style::default()
                        .bg(ratatui::style::Color::Indexed(238))
                        .fg(ratatui::style::Color::Indexed(255)),
                );
                let new_row = cr.min(ta.lines().len().saturating_sub(1));
                let new_col = cc.min(ta.lines().get(new_row).map(|l| l.chars().count()).unwrap_or(0));
                ta.move_cursor(tui_textarea::CursorMove::Jump(new_row as u16, new_col as u16));
                o.textarea = ta;
            }
            o.dirty = false;
        }
    }

    pub fn after_edit(&mut self, path: &Path) {
        if let Some(o) = self.open.as_mut() {
            o.dirty = false;
        }
        self.suppress(path);
        self.vault.reload(path);
        self.open_path(path);
    }

    fn quit(&mut self) {
        self.save_open();
        self.should_quit = true;
    }

    pub fn on_external_change(&mut self, path: &Path) {
        let is_md = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if !is_md || self.is_suppressed(path) {
            return;
        }
        if path.exists() {
            self.vault.reload(path);
            self.reload_open_if(path);
            self.status = Some("vault updated".to_string());
        } else {
            self.vault.remove(path);
            self.status = Some("note removed".to_string());
        }
    }

    fn reload_open_if(&mut self, path: &Path) {
        let matches = self
            .open
            .as_ref()
            .and_then(|o| self.vault.notes.get(o.idx))
            .map(|n| n.path.as_path())
            == Some(path);
        if matches
            && let Some(o) = self.open.as_ref()
            && o.dirty
        {
            return;
        }
        if matches
            && let Ok(src) = std::fs::read_to_string(path)
        {
            let mut lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
            if lines.is_empty() {
                lines.push(String::new());
            }
            let mut textarea = TextArea::new(lines);
            textarea.set_cursor_line_style(ratatui::style::Style::default());
            textarea.set_selection_style(
                ratatui::style::Style::default()
                    .bg(ratatui::style::Color::Indexed(238))
                    .fg(ratatui::style::Color::Indexed(255)),
            );
            if let Some(o) = self.open.as_mut() {
                let view = o.view;
                o.textarea = textarea;
                o.render = None;
                o.mode = EditMode::Normal;
                o.view = view;
            }
        }
    }

    fn suppress(&mut self, path: &Path) {
        self.suppress.insert(path.to_path_buf(), Instant::now());
    }

    fn is_suppressed(&mut self, path: &Path) -> bool {
        match self.suppress.get(path) {
            Some(at) if at.elapsed() < SUPPRESS_WINDOW => true,
            Some(_) => {
                self.suppress.remove(path);
                false
            }
            None => false,
        }
    }

    fn dispatch_leader(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('e') => {
                self.open_sidebar_focused();
                true
            }
            KeyCode::Char(' ') => {
                self.open_link_or_create_search();
                true
            }
            KeyCode::Char('g') => {
                self.open_graph();
                true
            }
            KeyCode::Char('x') => {
                self.toggle_checkbox();
                true
            }
            _ => false,
        }
    }

    fn toggle_checkbox(&mut self) {
        let Some(o) = self.open.as_mut() else { return };
        let (row, col) = o.textarea.cursor();
        let line = o.textarea.lines().get(row).cloned().unwrap_or_default();
        let Some(new_line) = toggle_checkbox_line(&line) else { return };
        o.textarea.move_cursor(CursorMove::Jump(row as u16, 0));
        o.textarea.delete_line_by_end();
        o.textarea.insert_str(new_line.clone());
        let new_col = col.min(new_line.chars().count());
        o.textarea.move_cursor(CursorMove::Jump(row as u16, new_col as u16));
        o.dirty = true;
    }

    fn is_leader_context(&self) -> bool {
        if self.switcher.is_some()
            || self.search.is_some()
            || self.prompt.is_some()
            || self.palette.is_some()
            || self.graph.is_some()
        {
            return false;
        }
        if self.focus == Focus::Sidebar {
            return true;
        }
        match self.open.as_ref() {
            Some(o) => matches!(o.view, ViewMode::Preview)
                || (o.view == ViewMode::Edit && o.mode == EditMode::Normal),
            None => true,
        }
    }

    fn open_sidebar_focused(&mut self) {
        if self.show_sidebar && self.focus == Focus::Sidebar {
            self.show_sidebar = false;
            self.focus = Focus::Reader;
            self.status = Some("sidebar hidden".to_string());
            return;
        }
        if !self.show_sidebar {
            self.save_open();
            self.show_sidebar = true;
        }
        self.focus = Focus::Sidebar;
        self.status = Some("sidebar".to_string());
    }

    fn delete_selected(&mut self) {
        let Some(note) = self.vault.notes.get(self.selected) else {
            return;
        };
        let path = note.path.clone();
        let title = note.title.clone();
        let was_open = self
            .open
            .as_ref()
            .and_then(|o| self.vault.notes.get(o.idx))
            .map(|n| n.path == path)
            .unwrap_or(false);
        self.suppress(&path);
        if std::fs::remove_file(&path).is_err() {
            self.status = Some("delete failed".to_string());
            return;
        }
        self.vault.remove(&path);
        if self.pinned.remove(&path) {
            save_pins(&self.vault.root, &self.pinned);
        }
        if was_open {
            self.open = None;
        }
        let n = self.vault.notes.len();
        if n == 0 {
            self.selected = 0;
        } else if self.selected >= n {
            self.selected = n - 1;
        }
        self.status = Some(format!("deleted {title}"));
    }

    fn open_rename_prompt(&mut self) {
        let Some(note) = self.vault.notes.get(self.selected) else {
            return;
        };
        let title = note.title.clone();
        let old_path = note.path.clone();
        self.prompt = Some(Prompt {
            title: format!("rename {title} to:"),
            input: title,
            kind: PromptKind::Rename { old_path },
        });
    }

    fn rename_to(&mut self, old_path: &Path, new_name: &str) {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return;
        }
        let parent = old_path.parent().unwrap_or(Path::new("."));
        let new_path = parent.join(format!("{new_name}.md"));
        if new_path.exists() {
            self.status = Some("name already in use".to_string());
            return;
        }
        self.save_open();
        let was_open = self
            .open
            .as_ref()
            .and_then(|o| self.vault.notes.get(o.idx))
            .map(|n| n.path.as_path() == old_path)
            .unwrap_or(false);
        self.suppress(old_path);
        self.suppress(&new_path);
        if std::fs::rename(old_path, &new_path).is_err() {
            self.status = Some("rename failed".to_string());
            return;
        }
        self.vault.remove(old_path);
        self.vault.reload(&new_path);
        if self.pinned.remove(old_path) {
            self.pinned.insert(new_path.clone());
            save_pins(&self.vault.root, &self.pinned);
        }
        self.status = Some(format!("renamed to {new_name}"));
        if was_open {
            self.open_path(&new_path);
        } else if let Some(idx) = self.vault.index_of(&new_path) {
            self.selected = idx;
        }
    }

    fn toggle_pin_selected(&mut self) {
        let Some(note) = self.vault.notes.get(self.selected) else {
            return;
        };
        let path = note.path.clone();
        let title = note.title.clone();
        if self.pinned.contains(&path) {
            self.pinned.remove(&path);
            self.status = Some(format!("unpinned {title}"));
        } else {
            self.pinned.insert(path);
            self.status = Some(format!("pinned {title}"));
        }
        save_pins(&self.vault.root, &self.pinned);
    }

    fn toggle_sidebar(&mut self) {
        self.show_sidebar = !self.show_sidebar;
        if self.show_sidebar {
            self.save_open();
            self.focus = Focus::Sidebar;
        } else {
            self.focus = Focus::Reader;
        }
    }

    fn visual_yank_line(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let (start_row, end_row) = match o.textarea.selection_range() {
            Some((s, e)) => {
                if (s.0, s.1) <= (e.0, e.1) {
                    (s.0, e.0)
                } else {
                    (e.0, s.0)
                }
            }
            None => {
                let (cr, _) = o.textarea.cursor();
                (cr, cr)
            }
        };
        let n_lines = o.textarea.lines().len();
        o.textarea.cancel_selection();
        o.textarea
            .move_cursor(CursorMove::Jump(start_row as u16, 0));
        o.textarea.start_selection();
        if end_row + 1 < n_lines {
            o.textarea
                .move_cursor(CursorMove::Jump((end_row + 1) as u16, 0));
        } else {
            o.textarea.move_cursor(CursorMove::Jump(end_row as u16, 0));
            o.textarea.move_cursor(CursorMove::End);
        }
        o.textarea.copy();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
        }
        o.textarea.cancel_selection();
        o.mode = EditMode::Normal;
    }

    fn visual_delete_line(&mut self) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let (start_row, end_row) = match o.textarea.selection_range() {
            Some((s, e)) => {
                if (s.0, s.1) <= (e.0, e.1) {
                    (s.0, e.0)
                } else {
                    (e.0, s.0)
                }
            }
            None => {
                let (cr, _) = o.textarea.cursor();
                (cr, cr)
            }
        };
        let n_lines = o.textarea.lines().len();
        o.textarea.cancel_selection();
        o.textarea
            .move_cursor(CursorMove::Jump(start_row as u16, 0));
        o.textarea.start_selection();
        if end_row + 1 < n_lines {
            o.textarea
                .move_cursor(CursorMove::Jump((end_row + 1) as u16, 0));
        } else {
            o.textarea.move_cursor(CursorMove::Jump(end_row as u16, 0));
            o.textarea.move_cursor(CursorMove::End);
        }
        o.textarea.cut();
        let yank = o.textarea.yank_text();
        if !yank.is_empty() {
            clipboard_set(&yank);
            o.dirty = true;
        }
        o.textarea.cancel_selection();
        o.mode = EditMode::Normal;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;

    fn app_with_one_note() -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("One.md"), "# One\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        (dir, App::new(vault))
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn suppressed_change_is_ignored() {
        let (dir, mut app) = app_with_one_note();
        let two = dir.path().join("Two.md");
        std::fs::write(&two, "# Two\n").unwrap();
        app.suppress(&two);
        app.on_external_change(&two);
        assert_eq!(app.vault.notes.len(), 1);
    }

    #[test]
    fn new_note_creates_and_opens() {
        let (dir, mut app) = app_with_one_note();
        app.new_note("Fresh Idea");
        assert!(dir.path().join("Fresh Idea.md").exists());
        let idx = app.open.as_ref().unwrap().idx;
        assert_eq!(app.vault.notes[idx].title, "Fresh Idea");
    }

    #[test]
    fn make_link_wraps_word_under_cursor() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "hello world\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        app.make_link();
        let line = app.open.as_ref().unwrap().textarea.lines()[0].clone();
        assert!(line.contains("[[hello]]"));
    }

    #[test]
    fn diw_deletes_inner_word() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "alpha beta gamma\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        // place cursor on "beta" (col 7 = 'e')
        if let Some(o) = app.open.as_mut() {
            o.textarea.move_cursor(CursorMove::Jump(0, 7));
        }
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('i')));
        app.on_key(key(KeyCode::Char('w')));
        let line = app.open.as_ref().unwrap().textarea.lines()[0].clone();
        assert!(!line.contains("beta"));
        assert!(line.contains("alpha"));
        assert!(line.contains("gamma"));
    }

    #[test]
    fn dd_deletes_current_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "first\nsecond\nthird\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        if let Some(o) = app.open.as_mut() {
            o.textarea.move_cursor(CursorMove::Jump(1, 0));
        }
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('d')));
        let joined = app.open.as_ref().unwrap().textarea.lines().join("\n");
        assert!(joined.contains("first"));
        assert!(!joined.contains("second"));
        assert!(joined.contains("third"));
    }

    #[test]
    fn yiw_yanks_inner_word_to_clipboard_buffer() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "alpha beta gamma\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        if let Some(o) = app.open.as_mut() {
            o.textarea.move_cursor(CursorMove::Jump(0, 7));
        }
        app.on_key(key(KeyCode::Char('y')));
        app.on_key(key(KeyCode::Char('i')));
        app.on_key(key(KeyCode::Char('w')));
        let yank = app.open.as_ref().unwrap().textarea.yank_text();
        assert_eq!(yank, "beta");
        // line should still contain beta (yank doesn't delete)
        let line = app.open.as_ref().unwrap().textarea.lines()[0].clone();
        assert!(line.contains("beta"));
    }

    #[test]
    fn visual_mode_y_yanks_selection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "hello world\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.open.as_ref().unwrap().mode, EditMode::Visual);
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Char('l')));
        app.on_key(key(KeyCode::Char('y')));
        assert_eq!(app.open.as_ref().unwrap().mode, EditMode::Normal);
        let yank = app.open.as_ref().unwrap().textarea.yank_text();
        assert!(yank.starts_with("hell"));
    }

    #[test]
    fn visual_y_uppercase_yanks_whole_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "first\nsecond\nthird\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT));
        let yank = app.open.as_ref().unwrap().textarea.yank_text();
        assert!(yank.contains("first"));
        assert!(yank.contains("second"));
    }

    #[test]
    fn visual_d_uppercase_deletes_whole_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "first\nsecond\nthird\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        app.on_key(key(KeyCode::Char('v')));
        app.on_key(key(KeyCode::Char('j')));
        app.on_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT));
        let joined = app.open.as_ref().unwrap().textarea.lines().join("\n");
        assert!(!joined.contains("first"));
        assert!(!joined.contains("second"));
        assert!(joined.contains("third"));
    }

    #[test]
    fn open_welcome_in_preview_lands_on_welcome() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Welcome.md"), "# Welcome\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_welcome_in_preview();
        let o = app.open.as_ref().unwrap();
        assert_eq!(app.vault.notes[o.idx].title, "Welcome");
        assert_eq!(o.view, ViewMode::Preview);
    }

    #[test]
    fn open_welcome_is_noop_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Other.md"), "# Other\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_welcome_in_preview();
        assert!(app.open.is_none());
    }

    #[test]
    fn generates_toc_with_grouped_links() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Welcome.md"), "# Welcome\n").unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/Sub.md"), "# Sub\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let app = App::new(vault);
        let toc = app.generate_toc();
        assert!(toc.contains("[[Welcome]]"));
        assert!(toc.contains("[[Sub]]"));
        assert!(toc.contains("## /"));
        assert!(toc.contains("## /sub/"));
    }

    #[test]
    fn link_or_create_grep_matches_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Apple.md"),
            "# Apple\nthe quick brown fox\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Other.md"), "# Other\nplain text\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_link_or_create_search();
        app.on_key(key(KeyCode::Char('f')));
        app.on_key(key(KeyCode::Char('o')));
        app.on_key(key(KeyCode::Char('x')));
        let p = app.switcher.as_ref().unwrap();
        assert!(!p.matches.is_empty());
        let top_item = &p.items[p.matches[0]];
        assert!(top_item.contains("Apple") || top_item.contains("fox"));
    }

    #[test]
    fn leader_e_toggles_sidebar_when_already_focused() {
        let (dir, mut app) = app_with_one_note();
        app.open_path(&dir.path().join("One.md"));
        // First leader-e: focus snaps to sidebar.
        app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert!(app.show_sidebar);
        assert_eq!(app.focus, Focus::Sidebar);
        // Second leader-e: sidebar hides, focus back to reader.
        app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert!(!app.show_sidebar);
        assert_eq!(app.focus, Focus::Reader);
    }

    #[test]
    fn leader_ctrl_e_opens_and_focuses_sidebar() {
        let (dir, mut app) = app_with_one_note();
        app.open_path(&dir.path().join("One.md"));
        app.toggle_sidebar();
        assert!(!app.show_sidebar);
        assert_eq!(app.focus, Focus::Reader);
        app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert!(app.show_sidebar);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn leader_space_two_step_opens_sidebar() {
        let (dir, mut app) = app_with_one_note();
        app.open_path(&dir.path().join("One.md"));
        app.set_leader(LeaderKind::Char(' '));
        app.toggle_sidebar();
        assert!(!app.show_sidebar);

        app.on_key(key(KeyCode::Char(' ')));
        assert!(app.leader_pending);
        app.on_key(key(KeyCode::Char('e')));
        assert!(!app.leader_pending);
        assert!(app.show_sidebar);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn v_in_visual_returns_to_normal() {
        let (dir, mut app) = app_with_one_note();
        app.open_path(&dir.path().join("One.md"));
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.open.as_ref().unwrap().mode, EditMode::Visual);
        app.on_key(key(KeyCode::Char('v')));
        assert_eq!(app.open.as_ref().unwrap().mode, EditMode::Normal);
    }

    #[test]
    fn tab_does_not_focus_hidden_sidebar() {
        let (dir, mut app) = app_with_one_note();
        app.open_path(&dir.path().join("One.md"));
        app.toggle_sidebar();
        assert!(!app.show_sidebar);
        assert_eq!(app.focus, Focus::Reader);
        app.on_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Reader);
        assert!(!app.show_sidebar);
    }

    #[test]
    fn ctrl_w_toggles_sidebar() {
        let (_dir, mut app) = app_with_one_note();
        assert!(app.show_sidebar);
        app.on_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert!(!app.show_sidebar);
        assert_eq!(app.focus, Focus::Reader);
        app.on_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert!(app.show_sidebar);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn alias_prompt_creates_aliased_link() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("A.md"), "personal\n").unwrap();
        let vault = Vault::load(dir.path().to_path_buf()).unwrap();
        let mut app = App::new(vault);
        app.open_path(&dir.path().join("A.md"));
        app.open_alias_prompt();
        let p = app.prompt.as_mut().unwrap();
        assert_eq!(p.input, "personal|personal");
        p.input = "Personal Info|personal".to_string();
        app.on_key(key(KeyCode::Enter));
        let line = app.open.as_ref().unwrap().textarea.lines()[0].clone();
        assert!(line.contains("[[Personal Info|personal]]"));
    }
}
