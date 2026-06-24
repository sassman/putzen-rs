//! Theme + small reusable rendering helpers.

use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg:           Color,
    pub bg_sel:       Color,
    pub bg_modal:     Color,
    pub fg:           Color,
    pub fg_bright:    Color,
    pub border:       Color,
    pub title:        Color,
    pub header:       Color,
    pub gutter_active:Color,
    pub gutter_marked:Color,
    pub hot:          Color,
    pub warm:         Color,
    pub ok:           Color,
    pub dim:          Color,
}

impl Theme {
    pub const GRUVBOX: Theme = Theme {
        bg:           Color::Rgb(0x28, 0x28, 0x28),
        bg_sel:       Color::Rgb(0x3c, 0x38, 0x36), // gruvbox bg1
        bg_modal:     Color::Rgb(0x1d, 0x20, 0x21),
        fg:           Color::Rgb(0xeb, 0xdb, 0xb2),
        fg_bright:    Color::Rgb(0xfb, 0xf1, 0xc7),
        border:       Color::Rgb(0x92, 0x83, 0x74),
        title:        Color::Rgb(0x8e, 0xc0, 0x7c),
        header:       Color::Rgb(0x83, 0xa5, 0x98),
        gutter_active:Color::Rgb(0xfa, 0xbd, 0x2f),
        gutter_marked:Color::Rgb(0xfe, 0x80, 0x19),
        hot:          Color::Rgb(0xfb, 0x49, 0x34),
        warm:         Color::Rgb(0xfe, 0x80, 0x19),
        ok:           Color::Rgb(0xb8, 0xbb, 0x26),
        dim:          Color::Rgb(0x92, 0x83, 0x74),
    };

    pub fn block_style(&self) -> Style { Style::default().fg(self.border).bg(self.bg) }
    pub fn title_style(&self) -> Style { Style::default().fg(self.title).add_modifier(Modifier::BOLD) }
    pub fn header_style(&self) -> Style { Style::default().fg(self.header).add_modifier(Modifier::BOLD) }
    pub fn gutter_active_style(&self) -> Style { Style::default().fg(self.gutter_active) }
    pub fn gutter_marked_style(&self) -> Style { Style::default().fg(self.gutter_marked) }
    pub fn body_style(&self) -> Style { Style::default().fg(self.fg).bg(self.bg) }
    pub fn modal_block_style(&self) -> Style { Style::default().fg(self.gutter_active).bg(self.bg_modal) }
    pub fn modal_body_style(&self) -> Style { Style::default().fg(self.fg_bright).bg(self.bg_modal) }
    pub fn dim_style(&self) -> Style { Style::default().fg(self.dim) }
}
