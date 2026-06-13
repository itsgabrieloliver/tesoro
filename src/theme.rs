use ratatui::style::{Color, Modifier, Style};

pub const EMPH: Color = Color::Indexed(255);
pub const TEXT: Color = Color::Indexed(252);
pub const MUTED: Color = Color::Indexed(245);
pub const FAINT: Color = Color::Indexed(240);
pub const ACCENT: Color = Color::Indexed(110);
pub const CODE: Color = Color::Indexed(180);
pub const SEL_BG: Color = Color::Indexed(238);
pub const WARN: Color = Color::Indexed(208);
pub const HL_BG: Color = Color::Indexed(58);
pub const INFO: Color = Color::Indexed(110);
pub const SUCCESS: Color = Color::Indexed(108);
pub const DANGER: Color = Color::Indexed(167);

const TAG_PALETTE: [Color; 6] = [
    Color::Indexed(108),
    Color::Indexed(110),
    Color::Indexed(180),
    Color::Indexed(139),
    Color::Indexed(143),
    Color::Indexed(173),
];

pub fn text() -> Style {
    Style::default().fg(TEXT)
}

pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

pub fn faint() -> Style {
    Style::default().fg(FAINT)
}

pub fn brand() -> Style {
    Style::default().fg(EMPH).add_modifier(Modifier::BOLD)
}

pub fn warn() -> Style {
    Style::default().fg(WARN).add_modifier(Modifier::BOLD)
}

pub fn border() -> Style {
    Style::default().fg(Color::Indexed(236))
}

pub fn statusbar() -> Style {
    Style::default().fg(MUTED).bg(Color::Indexed(236))
}

pub fn selected() -> Style {
    Style::default()
        .fg(EMPH)
        .bg(SEL_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn link() -> Style {
    Style::default().fg(ACCENT)
}

pub fn phantom() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::DIM | Modifier::ITALIC)
}

pub fn highlight() -> Style {
    Style::default().fg(EMPH).bg(HL_BG)
}

pub fn tag_for(name: &str) -> Style {
    let mut h: usize = 0;
    for b in name.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as usize);
    }
    Style::default().fg(TAG_PALETTE[h % TAG_PALETTE.len()])
}

pub fn named_color(spec: &str) -> Option<Color> {
    if let Some(hex) = spec.strip_prefix('#') {
        if hex.len() == 6
            && let Ok(v) = u32::from_str_radix(hex, 16)
        {
            return Some(Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8));
        }
        return None;
    }
    let c = match spec.to_ascii_lowercase().as_str() {
        "red" | "danger" | "error" => DANGER,
        "green" | "success" => SUCCESS,
        "blue" | "info" | "accent" => ACCENT,
        "yellow" => Color::Indexed(143),
        "orange" | "warn" => WARN,
        "purple" | "violet" => Color::Indexed(139),
        "gray" | "grey" | "muted" => MUTED,
        "white" => EMPH,
        _ => return None,
    };
    Some(c)
}

pub fn callout(kind: &str) -> (char, Color) {
    match kind {
        "tip" | "hint" | "success" | "check" | "done" => ('✓', SUCCESS),
        "warning" | "caution" | "attention" => ('⚠', WARN),
        "danger" | "error" | "bug" | "fail" | "failure" => ('✗', DANGER),
        "question" | "help" | "faq" => ('?', Color::Indexed(139)),
        "quote" | "cite" => ('❝', MUTED),
        "example" => ('≡', Color::Indexed(139)),
        _ => ('ℹ', INFO),
    }
}

pub fn code() -> Style {
    Style::default().fg(CODE)
}

pub fn link_selected() -> Style {
    Style::default()
        .fg(Color::Indexed(16))
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}
