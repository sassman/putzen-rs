//! Theme + small reusable rendering helpers.

use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg: Color,
    pub bg_sel: Color,
    pub bg_modal: Color,
    pub fg: Color,
    pub fg_bright: Color,
    pub border: Color,
    pub title: Color,
    pub header: Color,
    pub gutter_active: Color,
    pub gutter_marked: Color,
    pub hot: Color,
    pub warm: Color,
    pub ok: Color,
    pub dim: Color,
}

impl Theme {
    pub const GRUVBOX: Theme = Theme {
        bg: Color::Rgb(0x28, 0x28, 0x28),
        bg_sel: Color::Rgb(0x3c, 0x38, 0x36), // gruvbox bg1
        bg_modal: Color::Rgb(0x1d, 0x20, 0x21),
        fg: Color::Rgb(0xeb, 0xdb, 0xb2),
        fg_bright: Color::Rgb(0xfb, 0xf1, 0xc7),
        border: Color::Rgb(0x92, 0x83, 0x74),
        title: Color::Rgb(0x8e, 0xc0, 0x7c),
        header: Color::Rgb(0x83, 0xa5, 0x98),
        gutter_active: Color::Rgb(0xfa, 0xbd, 0x2f),
        gutter_marked: Color::Rgb(0xfe, 0x80, 0x19),
        hot: Color::Rgb(0xfb, 0x49, 0x34),
        warm: Color::Rgb(0xfe, 0x80, 0x19),
        ok: Color::Rgb(0xb8, 0xbb, 0x26),
        dim: Color::Rgb(0x92, 0x83, 0x74),
    };

    pub fn block_style(&self) -> Style {
        Style::default().fg(self.border).bg(self.bg)
    }
    pub fn title_style(&self) -> Style {
        Style::default().fg(self.title).add_modifier(Modifier::BOLD)
    }
    pub fn header_style(&self) -> Style {
        Style::default()
            .fg(self.header)
            .add_modifier(Modifier::BOLD)
    }
    pub fn gutter_active_style(&self) -> Style {
        Style::default().fg(self.gutter_active)
    }
    pub fn gutter_marked_style(&self) -> Style {
        Style::default().fg(self.gutter_marked)
    }
    pub fn body_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }
    pub fn modal_block_style(&self) -> Style {
        Style::default().fg(self.gutter_active).bg(self.bg_modal)
    }
    pub fn modal_body_style(&self) -> Style {
        Style::default().fg(self.fg_bright).bg(self.bg_modal)
    }
    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.dim)
    }

    /// Smooth heat-map between `ok`, `warm`, and `hot` keyed by `t ∈ [0, 1]`:
    /// `0.0` is pure `ok` (low score), `0.5` is pure `warm`, `1.0` is pure
    /// `hot` (highest score in the visible set).  Values outside the range
    /// are clamped.  Used for the score-bar colour.
    pub fn score_color(&self, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.5 {
            lerp_rgb(self.ok, self.warm, t * 2.0)
        } else {
            lerp_rgb(self.warm, self.hot, (t - 0.5) * 2.0)
        }
    }
}

fn lerp_rgb(a: Color, b: Color, t: f64) -> Color {
    let (ar, ag, ab) = rgb(a);
    let (br, bg, bb) = rgb(b);
    let mix = |x: u8, y: u8| ((x as f64) + ((y as f64) - (x as f64)) * t).round() as u8;
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

fn rgb(c: Color) -> (u8, u8, u8) {
    if let Color::Rgb(r, g, b) = c {
        (r, g, b)
    } else {
        // Theme palette is RGB by construction; non-RGB only reachable if
        // someone passes a named colour, in which case we render a flat grey.
        (0x80, 0x80, 0x80)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_color_endpoints_match_palette() {
        let t = Theme::GRUVBOX;
        assert_eq!(t.score_color(0.0), t.ok);
        assert_eq!(t.score_color(0.5), t.warm);
        assert_eq!(t.score_color(1.0), t.hot);
    }

    #[test]
    fn score_color_clamps_out_of_range() {
        let t = Theme::GRUVBOX;
        assert_eq!(t.score_color(-1.0), t.ok);
        assert_eq!(t.score_color(2.0), t.hot);
    }

    #[test]
    fn score_color_blends_between_anchors() {
        let t = Theme::GRUVBOX;
        // Quarter-way: half-way between ok and warm.
        let q = t.score_color(0.25);
        let (qr, qg, qb) = rgb(q);
        let (ok_r, ok_g, ok_b) = rgb(t.ok);
        let (wa_r, wa_g, wa_b) = rgb(t.warm);
        let mid = |a: u8, b: u8| ((a as u16 + b as u16) / 2) as u8;
        // Allow ±1 for round-off.
        assert!((qr as i16 - mid(ok_r, wa_r) as i16).abs() <= 1);
        assert!((qg as i16 - mid(ok_g, wa_g) as i16).abs() <= 1);
        assert!((qb as i16 - mid(ok_b, wa_b) as i16).abs() <= 1);
    }
}
