// Premade palette + RGB slider math for the Themes color picker.
// Pure data + geometry; no rendering or key handling.

#![allow(dead_code)]

use ratatui::layout::Rect;

use crate::theme::ColorDef;

pub const SWATCH_COLS: u16 = 12;
pub const SWATCH_ROWS: u16 = 2;
pub const SWATCH_CELL_W: u16 = 2;
pub const SWATCH_GAP: u16 = 1;
pub const SLIDER_BAR_W: u16 = 16;

pub struct Swatch {
    pub label: &'static str,
    pub color: ColorDef,
}

pub fn premade() -> Vec<Swatch> {
    let named = [
        "black",
        "dark_gray",
        "gray",
        "white",
        "red",
        "light_red",
        "green",
        "light_green",
        "yellow",
        "light_yellow",
        "blue",
        "light_blue",
        "magenta",
        "light_magenta",
        "cyan",
        "light_cyan",
    ];
    let mut out: Vec<Swatch> = named
        .iter()
        .map(|&label| Swatch {
            label,
            color: ColorDef::Named(label.to_string()),
        })
        .collect();
    out.extend([
        Swatch {
            label: "firebrick",
            color: ColorDef::Rgb(178, 34, 34),
        },
        Swatch {
            label: "orange",
            color: ColorDef::Rgb(255, 136, 0),
        },
        Swatch {
            label: "pink",
            color: ColorDef::Rgb(255, 105, 180),
        },
        Swatch {
            label: "brown",
            color: ColorDef::Rgb(139, 69, 19),
        },
        Swatch {
            label: "purple",
            color: ColorDef::Rgb(124, 58, 237),
        },
        Swatch {
            label: "teal",
            color: ColorDef::Rgb(0, 128, 128),
        },
        Swatch {
            label: "navy",
            color: ColorDef::Rgb(0, 0, 128),
        },
        Swatch {
            label: "olive",
            color: ColorDef::Rgb(128, 128, 0),
        },
    ]);
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
}

pub fn to_rgb(def: &ColorDef) -> (u8, u8, u8) {
    match def {
        ColorDef::Rgb(r, g, b) => (*r, *g, *b),
        ColorDef::Named(name) => match name.to_ascii_lowercase().as_str() {
            "black" => (0, 0, 0),
            "dark_gray" => (85, 85, 85),
            "gray" => (170, 170, 170),
            "white" => (255, 255, 255),
            "red" => (205, 0, 0),
            "light_red" => (255, 85, 85),
            "green" => (0, 205, 0),
            "light_green" => (85, 255, 85),
            "yellow" => (205, 205, 0),
            "light_yellow" => (255, 255, 85),
            "blue" => (0, 0, 238),
            "light_blue" => (85, 85, 255),
            "magenta" => (205, 0, 205),
            "light_magenta" => (255, 85, 255),
            "cyan" => (0, 205, 205),
            "light_cyan" => (85, 255, 255),
            "dark_red" => (139, 0, 0),
            _ => (255, 255, 255),
        },
    }
}

fn clamp_u8(v: u8, delta: i16) -> u8 {
    (i32::from(v) + i32::from(delta)).clamp(0, 255) as u8
}

pub fn adjust_channel(def: &ColorDef, ch: Channel, delta: i16) -> ColorDef {
    let (r, g, b) = to_rgb(def);
    match ch {
        Channel::R => ColorDef::Rgb(clamp_u8(r, delta), g, b),
        Channel::G => ColorDef::Rgb(r, clamp_u8(g, delta), b),
        Channel::B => ColorDef::Rgb(r, g, clamp_u8(b, delta)),
    }
}

pub fn slider_value_at_x(x_in_bar: u16, bar_w: u16) -> u8 {
    if bar_w <= 1 {
        return 0;
    }
    let x = u32::from(x_in_bar.min(bar_w - 1));
    (x * 255 / u32::from(bar_w - 1)) as u8
}

fn swatch_count() -> usize {
    usize::from(SWATCH_COLS) * usize::from(SWATCH_ROWS)
}

pub fn swatch_rect(palette_area: Rect, i: usize) -> Option<Rect> {
    if i >= swatch_count() {
        return None;
    }
    let col = (i % usize::from(SWATCH_COLS)) as u16;
    let row = (i / usize::from(SWATCH_COLS)) as u16;
    let stride = SWATCH_CELL_W.saturating_add(SWATCH_GAP);
    let x_off = col.saturating_mul(stride);
    let y_off = row;
    if y_off >= palette_area.height {
        return None;
    }
    if x_off.saturating_add(SWATCH_CELL_W) > palette_area.width {
        return None;
    }
    Some(Rect::new(
        palette_area.x.saturating_add(x_off),
        palette_area.y.saturating_add(y_off),
        SWATCH_CELL_W,
        1,
    ))
}

pub fn hit_swatch(palette_area: Rect, x: u16, y: u16) -> Option<usize> {
    (0..swatch_count()).find(|&i| {
        swatch_rect(palette_area, i).is_some_and(|r| {
            x >= r.x
                && x < r.x.saturating_add(r.width)
                && y >= r.y
                && y < r.y.saturating_add(r.height)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorDef;
    use ratatui::layout::Rect;

    #[test]
    fn palette_has_24_unique_entries() {
        let p = premade();
        assert_eq!(p.len(), 24);
        let mut labels: Vec<&str> = p.iter().map(|s| s.label).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), 24);
    }

    #[test]
    fn to_rgb_named_red_is_not_white() {
        let (r, g, b) = to_rgb(&ColorDef::Named("red".into()));
        assert!(r > 100 && g < 50 && b < 50, "{r},{g},{b}");
    }

    #[test]
    fn adjust_channel_saturates() {
        let c = ColorDef::Rgb(250, 0, 5);
        assert_eq!(adjust_channel(&c, Channel::R, 8), ColorDef::Rgb(255, 0, 5));
        assert_eq!(adjust_channel(&c, Channel::B, -8), ColorDef::Rgb(250, 0, 0));
    }

    #[test]
    fn slider_x_to_value_ends() {
        assert_eq!(slider_value_at_x(0, 16), 0);
        assert_eq!(slider_value_at_x(15, 16), 255);
    }

    #[test]
    fn hit_swatch_round_trips_indices() {
        let area = Rect::new(10, 5, 40, 2);
        let r0 = swatch_rect(area, 0).expect("swatch 0 fits");
        assert_eq!(hit_swatch(area, r0.x, r0.y), Some(0));
        assert_eq!(hit_swatch(area, r0.x + 1, r0.y), Some(0));
        // Gap column after the 2-cell body is not a hit.
        assert_eq!(hit_swatch(area, r0.x + SWATCH_CELL_W, r0.y), None);

        let r12 = swatch_rect(area, 12).expect("swatch 12 fits");
        assert_eq!(hit_swatch(area, r12.x, r12.y), Some(12));
        assert_eq!(hit_swatch(area, r12.x + 1, r12.y), Some(12));
    }
}
