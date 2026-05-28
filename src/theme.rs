use ratatui::style::{Color, Modifier, Style};

pub const EMPH: Color = Color::Indexed(255);
pub const TEXT: Color = Color::Indexed(252);
pub const MUTED: Color = Color::Indexed(245);
pub const FAINT: Color = Color::Indexed(240);
pub const ACCENT: Color = Color::Indexed(110);
pub const GREEN: Color = Color::Indexed(108);
pub const CODE: Color = Color::Indexed(180);
pub const SEL_BG: Color = Color::Indexed(238);

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

pub fn tag() -> Style {
    Style::default().fg(GREEN)
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
