// Map Preview for Horizon: a filled map's picture in its tooltip, and a small
// copy of it in its inventory slot instead of the plain map icon. Inspired by
// the Map Tooltip and Map in Slot mods, written from scratch on Horizon's hook
// points (none of their code). This file is both the mod's Cargo library (its
// tests) and the module the bundle adds to the game.
//
// The pictures come from the maps this client already has (the ones the
// server sent, as for any map you carry); a map the client hasn't been sent
// shows no preview. They are drawn as filled rectangles, one per run of
// same-coloured pixels.
use std::cell::RefCell;

/// The mod's id (its settings folder), name and version.
pub const ID: &str = "map_preview";
pub const NAME: &str = "Map Preview";
pub const VERSION: &str = "1.0.0";
pub const DESCRIPTION: &str =
    "A filled map's picture in its tooltip and in its inventory slot, so you can tell your maps apart.";

/// A map's side, in pixels.
pub const MAP_SIZE: usize = 128;
/// The map paper behind the picture (vanilla's map background).
pub const PAPER: u32 = 0xFF_DCC9A0;
/// The paper's darker edge.
pub const PAPER_EDGE: u32 = 0xFF_9C8459;
/// Space above the tooltip picture, in GUI pixels.
pub const TOOLTIP_GAP: f32 = 2.0;
/// A slot's icon, in GUI pixels.
pub const SLOT_SIZE: f32 = 16.0;
/// Map rotation in inventory slot as a map item: +3.5 degrees around (x + 7.5, y + 7.5).
pub const MAP_ITEM_ROTATION_RAD: f32 = 3.5 * std::f32::consts::PI / 180.0;
/// The map picture side inside the map item cutout overlay, in GUI pixels.
pub const MAP_ITEM_INNER_SIZE: f32 = 13.0;

/// One option of the settings page: its file, its label, the values a click
/// cycles through (with their names) and the default.
pub struct Opt {
    pub key: &'static str,
    pub label: &'static str,
    pub choices: &'static [(u16, &'static str)],
    pub default: u16,
}

const ON_OFF: &[(u16, &str)] = &[(0, "OFF"), (1, "ON")];
const TOOLTIP: &[(u16, &str)] = &[(0, "OFF"), (1, "Always"), (2, "Hold Shift")];
/// The tooltip picture's side in GUI pixels.
const TOOLTIP_SIZE: &[(u16, &str)] = &[(64, "Small"), (96, "Medium"), (128, "Large")];
const BACKGROUND: &[(u16, &str)] = &[(0, "OFF"), (1, "Paper"), (2, "Map background")];
const SLOT_STYLE: &[(u16, &str)] = &[(0, "Full slot"), (1, "Border"), (2, "As map item")];
const SLOT_RES: &[(u16, &str)] = &[(0, "Pixelated"), (1, "High")];

pub const OPTS: [Opt; 7] = [
    Opt { key: "tooltip", label: "Tooltip preview", choices: TOOLTIP, default: 1 },
    Opt { key: "tooltip_size", label: "Tooltip size", choices: TOOLTIP_SIZE, default: 96 },
    Opt { key: "paper", label: "Tooltip background", choices: BACKGROUND, default: 2 },
    Opt { key: "slot", label: "Slot preview", choices: ON_OFF, default: 1 },
    Opt { key: "slot_style", label: "Slot style", choices: SLOT_STYLE, default: 2 },
    Opt { key: "slot_res", label: "Slot resolution", choices: SLOT_RES, default: 1 },
    Opt { key: "locked_glass", label: "Locked map glass", choices: ON_OFF, default: 1 },
];

/// Indexes into `OPTS`.
pub mod o {
    pub const TOOLTIP: usize = 0;
    pub const TOOLTIP_SIZE: usize = 1;
    pub const PAPER: usize = 2;
    pub const SLOT: usize = 3;
    pub const SLOT_STYLE: usize = 4;
    pub const SLOT_RES: usize = 5;
    pub const LOCKED_GLASS: usize = 6;
}

/// The label of the button after the options.
pub const RESET_LABEL: &str = "Reset to defaults";

/// The settings, one file each in the mod's data folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    values: [u16; OPTS.len()],
}

impl Default for Settings {
    fn default() -> Self {
        Self { values: OPTS.map(|opt| opt.default) }
    }
}

impl Settings {
    /// From the stored values (`get` reads one key's file), the defaults for
    /// missing or unknown ones.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut s = Self::default();
        for (i, opt) in OPTS.iter().enumerate() {
            let value = get(opt.key).and_then(|v| v.trim().parse::<u16>().ok());
            if let Some(v) = value
                && opt.choices.iter().any(|c| c.0 == v)
            {
                s.values[i] = v;
            }
        }
        s
    }

    pub fn get(&self, i: usize) -> u16 {
        self.values[i]
    }

    /// The tooltip picture's side, if the tooltip shows one now (`shift`:
    /// whether Shift is held).
    pub fn tooltip_side(&self, shift: bool) -> Option<f32> {
        match self.get(o::TOOLTIP) {
            1 => Some(f32::from(self.get(o::TOOLTIP_SIZE))),
            2 if shift => Some(f32::from(self.get(o::TOOLTIP_SIZE))),
            _ => None,
        }
    }

    /// The settings page's buttons: the options, then reset.
    pub fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = OPTS
            .iter()
            .zip(self.values)
            .map(|(opt, v)| {
                let name = opt.choices.iter().find(|c| c.0 == v).map_or("?", |c| c.1);
                format!("{}: {name}", opt.label)
            })
            .collect();
        labels.push(RESET_LABEL.into());
        labels
    }

    /// A click on button `i` of `labels`: what to store, key by key.
    pub fn click(&mut self, i: usize) -> Vec<(&'static str, String)> {
        if i == OPTS.len() {
            *self = Self::default();
            return OPTS.iter().zip(self.values).map(|(opt, v)| (opt.key, v.to_string())).collect();
        }
        let Some(opt) = OPTS.get(i) else {
            return Vec::new();
        };
        let at = opt.choices.iter().position(|c| c.0 == self.values[i]).unwrap_or(0);
        self.values[i] = opt.choices[(at + 1) % opt.choices.len()].0;
        vec![(opt.key, self.values[i].to_string())]
    }
}

/// A map pixel's colour as ARGB (`MapColor.getColorFromPackedId`): the top six
/// bits pick `palette`'s base colour (`MapColor.col`), the low two its shade.
/// None for the empty colour, which shows the paper.
pub fn argb(packed: u8, palette: &[u32]) -> Option<u32> {
    let id = usize::from(packed >> 2);
    if id == 0 || id >= palette.len() {
        return None;
    }
    let modifier = [180u32, 220, 255, 135][usize::from(packed & 3)];
    let col = palette[id];
    let ch = |s: u32| (col >> s & 255) * modifier / 255;
    Some(0xFF00_0000 | ch(16) << 16 | ch(8) << 8 | ch(0))
}

/// One filled rectangle: `[x, y, w, h]` in GUI pixels, and its ARGB colour.
pub type Rect = ([f32; 4], u32);
/// One filled quadrilateral: 4 corner points `[x, y]` in GUI pixels, and its ARGB colour.
pub type Quad = ([[f32; 2]; 4], u32);

/// The map's picture at `[x, y]`, `side` GUI pixels square, sampled to
/// `cells` x `cells` (each the map pixel at its centre), as rectangles: one
/// per run of same-coloured cells in a row. Empty pixels are left out.
pub fn picture(colors: &[u8], palette: &[u32], at: [f32; 2], side: f32, cells: usize) -> Vec<Rect> {
    let mut rects = Vec::new();
    if colors.len() < MAP_SIZE * MAP_SIZE || cells == 0 {
        return rects;
    }
    let cell = side / cells as f32;
    let source = |c: usize| (c * MAP_SIZE + MAP_SIZE / 2) / cells;
    for row in 0..cells {
        let sy = source(row);
        let mut run: Option<(usize, u32)> = None;
        for col in 0..=cells {
            let color = (col < cells).then(|| argb(colors[sy * MAP_SIZE + source(col)], palette)).flatten();
            match run {
                Some((_, c)) if Some(c) == color => continue,
                Some((start, c)) => {
                    let x = at[0] + start as f32 * cell;
                    rects.push(([x, at[1] + row as f32 * cell, (col - start) as f32 * cell, cell], c));
                    run = None;
                }
                None => {}
            }
            if let Some(c) = color {
                run = Some((col, c));
            }
        }
    }
    rects
}

/// Outer padding around the map for framed tooltip background (bg == 2).
/// Follows vanilla MapItemRenderer frame margin: ~7 units per 128 units.
pub fn tooltip_pad(side: f32) -> f32 {
    (side * 7.0 / 128.0).round().max(2.0)
}

/// The tooltip space for a map, `[width, height]`.
/// When `framed` (bg == 2), includes padding around the map on all sides.
pub fn tooltip_space(side: f32, framed: bool) -> [f32; 2] {
    if framed {
        let total = side + 2.0 * tooltip_pad(side);
        [total, total + TOOLTIP_GAP]
    } else {
        [side, side + TOOLTIP_GAP]
    }
}

/// The tooltip's picture: background (0: none, 1: paper, 2: paper fallback), then map.
/// Empty/unexplored map pixels let the background show through.
pub fn tooltip_rects(colors: &[u8], palette: &[u32], at: [f32; 2], side: f32, bg: u16) -> Vec<Rect> {
    let top = [at[0], at[1] + TOOLTIP_GAP];
    let mut rects = Vec::new();
    let (map_at, map_side) = if bg == 2 {
        let pad = tooltip_pad(side);
        let total = side + 2.0 * pad;
        rects.push(([top[0], top[1], total, total], PAPER));
        ([top[0] + pad, top[1] + pad], side)
    } else {
        if bg == 1 {
            rects.push(([top[0], top[1], side, side], PAPER));
        }
        (top, side)
    };
    let cells = (map_side.round() as usize).min(MAP_SIZE);
    rects.extend(picture(colors, palette, map_at, map_side, cells));
    rects
}

/// A slot's picture at the icon's top-left `[x, y]`: the whole slot, or
/// inside a one-pixel paper edge.
pub fn slot_rects(
    colors: &[u8],
    palette: &[u32],
    at: [f32; 2],
    edged: bool,
    paper: bool,
    high_res: bool,
) -> Vec<Rect> {
    let mut rects = Vec::new();
    let (inner, side) = if edged {
        rects.push(([at[0], at[1], SLOT_SIZE, SLOT_SIZE], PAPER_EDGE));
        ([at[0] + 1.0, at[1] + 1.0], SLOT_SIZE - 2.0)
    } else {
        (at, SLOT_SIZE)
    };
    if paper || edged {
        rects.push(([inner[0], inner[1], side, side], PAPER));
    }
    let cells = if high_res { MAP_SIZE } else { side as usize };
    rects.extend(picture(colors, palette, inner, side, cells));
    rects
}

/// Rotates a 2D point `[px, py]` around center `[cx, cy]` by angle with `sin` and `cos`.
#[inline]
pub fn rotate_point(pt: [f32; 2], center: [f32; 2], sin: f32, cos: f32) -> [f32; 2] {
    let dx = pt[0] - center[0];
    let dy = pt[1] - center[1];
    [center[0] + dx * cos - dy * sin, center[1] + dx * sin + dy * cos]
}

/// The paper backing quad tilted for "As map item".
pub fn tilted_paper_quad(at: [f32; 2]) -> Quad {
    let center = [at[0] + 7.5, at[1] + 7.5];
    let (sin, cos) = MAP_ITEM_ROTATION_RAD.sin_cos();
    let half = MAP_ITEM_INNER_SIZE / 2.0;
    let corners = [
        rotate_point([center[0] - half, center[1] - half], center, sin, cos),
        rotate_point([center[0] + half, center[1] - half], center, sin, cos),
        rotate_point([center[0] + half, center[1] + half], center, sin, cos),
        rotate_point([center[0] - half, center[1] + half], center, sin, cos),
    ];
    (corners, PAPER)
}

/// The map's picture tilted for "As map item" sampled to `cells` x `cells`.
pub fn tilted_picture(
    colors: &[u8],
    palette: &[u32],
    at: [f32; 2],
    cells: usize,
) -> Vec<Quad> {
    let mut quads = Vec::new();
    if colors.len() < MAP_SIZE * MAP_SIZE || cells == 0 {
        return quads;
    }
    let center = [at[0] + 7.5, at[1] + 7.5];
    let (sin, cos) = MAP_ITEM_ROTATION_RAD.sin_cos();
    let half = MAP_ITEM_INNER_SIZE / 2.0;
    let origin = [center[0] - half, center[1] - half];
    let cell = MAP_ITEM_INNER_SIZE / cells as f32;
    let source = |c: usize| (c * MAP_SIZE + MAP_SIZE / 2) / cells;

    for row in 0..cells {
        let sy = source(row);
        let mut run: Option<(usize, u32)> = None;
        for col in 0..=cells {
            let color = (col < cells)
                .then(|| argb(colors[sy * MAP_SIZE + source(col)], palette))
                .flatten();
            match run {
                Some((_, c)) if Some(c) == color => continue,
                Some((start, c)) => {
                    let x0 = origin[0] + start as f32 * cell;
                    let x1 = origin[0] + col as f32 * cell;
                    let y0 = origin[1] + row as f32 * cell;
                    let y1 = origin[1] + (row + 1) as f32 * cell;
                    let corners = [
                        rotate_point([x0, y0], center, sin, cos),
                        rotate_point([x1, y0], center, sin, cos),
                        rotate_point([x1, y1], center, sin, cos),
                        rotate_point([x0, y1], center, sin, cos),
                    ];
                    quads.push((corners, c));
                    run = None;
                }
                None => {}
            }
            if let Some(c) = color {
                run = Some((col, c));
            }
        }
    }
    quads
}

/// Pixel runs for frame 16 (16x16).
pub const FRAME_16: &[(u8, u8, u8, u32)] = &[
    (1, 0, 8, 0xFF947F5D), (1, 1, 1, 0xFF947F5D), (2, 1, 6, 0xFFFFF0D1), (8, 1, 1, 0xFFF2E2C2),
    (9, 1, 6, 0xFF947F5D), (15, 1, 1, 0xFF877251), (1, 2, 1, 0xFF947F5D), (2, 2, 1, 0xFFFFF0D1),
    (3, 2, 1, 0xA0D4C09D), (9, 2, 1, 0xA0BAA57F), (10, 2, 1, 0xA0D4C09D), (11, 2, 1, 0xFFD4C09D),
    (12, 2, 3, 0xFFEBD7B2), (15, 2, 1, 0xFF877251), (1, 3, 1, 0xFF947F5D), (2, 3, 1, 0xFFFFF0D1),
    (13, 3, 1, 0xA0D4C09D), (14, 3, 1, 0xFFF2E2C2), (15, 3, 1, 0xFF877251), (1, 4, 1, 0xFF947F5D),
    (2, 4, 1, 0xFFFFF0D1), (14, 4, 1, 0xFFF2E2C2), (15, 4, 1, 0xFF877251), (1, 5, 1, 0xFF947F5D),
    (2, 5, 1, 0xFFF2E2C2), (3, 5, 1, 0xA0D4C09D), (14, 5, 1, 0xFFD4C09D), (15, 5, 1, 0xFF736041),
    (1, 6, 1, 0xFF877251), (2, 6, 1, 0xA0F2E2C2), (3, 6, 1, 0xA0BAA57F), (14, 6, 1, 0xFFEBD7B2),
    (15, 6, 1, 0xFF736041), (0, 7, 1, 0xFF877251), (1, 7, 1, 0xFFF2E2C2), (2, 7, 1, 0xA0D4C09D),
    (14, 7, 1, 0xFFF2E2C2), (15, 7, 1, 0xFF736041), (0, 8, 1, 0xFF947F5D), (1, 8, 1, 0xFFFFF0D1),
    (13, 8, 1, 0xA0BAA57F), (14, 8, 1, 0xFFD4C09D), (15, 8, 1, 0xFF736041), (0, 9, 1, 0xFF947F5D),
    (1, 9, 1, 0xFFFFF0D1), (13, 9, 1, 0xFFD4C09D), (14, 9, 1, 0xFF736041), (0, 10, 1, 0xFF947F5D),
    (1, 10, 1, 0xFFFFF0D1), (13, 10, 1, 0xFFEBD7B2), (14, 10, 1, 0xFF736041), (0, 11, 1, 0xFF947F5D),
    (1, 11, 1, 0xFFD4C09D), (12, 11, 1, 0xA0D4C09D), (13, 11, 1, 0xA0F2E2C2), (14, 11, 1, 0xFF736041),
    (0, 12, 1, 0xFF877251), (1, 12, 1, 0xFFEBD7B2), (2, 12, 1, 0xFFD4C09D), (12, 12, 1, 0xA0F2E2C2),
    (13, 12, 1, 0xFFFFF0D1), (14, 12, 1, 0xFF736041), (0, 13, 1, 0xFF877251), (1, 13, 1, 0xFFF2E2C2),
    (2, 13, 1, 0xFFEBD7B2), (3, 13, 1, 0xA0EBD7B2), (4, 13, 2, 0xFFEBD7B2), (6, 13, 1, 0xFFD4C09D),
    (7, 13, 1, 0xA0D4C09D), (11, 13, 1, 0xA0D4C09D), (12, 13, 2, 0xFFFFF0D1), (14, 13, 1, 0xFF736041),
    (0, 14, 2, 0xFF877251), (2, 14, 1, 0xFF736041), (3, 14, 1, 0xFF877251), (4, 14, 3, 0xFF736041),
    (7, 14, 1, 0xFFBAA57F), (8, 14, 1, 0xFFD4C09D), (9, 14, 1, 0xFFEBD7B2), (10, 14, 1, 0xA0F2E2C2),
    (11, 14, 2, 0xFFFFF0D1), (13, 14, 1, 0xFFF2E2C2), (14, 14, 1, 0xFF736041), (7, 15, 6, 0xFF736041),
    (13, 15, 2, 0xFF877251),
];

/// Pixel runs for frame 32 (32x32).
pub const FRAME_32: &[(u8, u8, u8, u32)] = &[
    (3, 0, 8, 0xFF947F5D), (3, 1, 1, 0xFF947F5D), (4, 1, 5, 0xFFFFF0D1), (9, 1, 2, 0xFFF2E2C2),
    (11, 1, 7, 0xFF947F5D), (3, 2, 1, 0xFF947F5D), (4, 2, 1, 0xFFFFF0D1), (5, 2, 1, 0xFFF2E2C2),
    (6, 2, 8, 0xFFFFF0D1), (14, 2, 3, 0xFFF2E2C2), (17, 2, 1, 0xFFEBD7B2), (18, 2, 7, 0xFF947F5D),
    (3, 3, 1, 0xFF947F5D), (4, 3, 1, 0xFFFFF0D1), (5, 3, 1, 0xFFF2E2C2), (6, 3, 1, 0xC8D4C09D),
    (17, 3, 1, 0xC8BAA57F), (18, 3, 5, 0xFFD4C09D), (23, 3, 2, 0xFFEBD7B2), (25, 3, 6, 0xFF947F5D),
    (31, 3, 1, 0xFF877251), (3, 4, 1, 0xFF947F5D), (4, 4, 1, 0xFFFFF0D1), (5, 4, 1, 0xC8D4C09D),
    (23, 4, 1, 0xC8D4C09D), (24, 4, 7, 0xFFEBD7B2), (31, 4, 1, 0xFF877251), (3, 5, 1, 0xFF947F5D),
    (4, 5, 1, 0xFFFFF0D1), (27, 5, 1, 0xC8D4C09D), (28, 5, 2, 0xFFF2E2C2), (30, 5, 1, 0xFFEBD7B2),
    (31, 5, 1, 0xFF877251), (3, 6, 1, 0xFF947F5D), (4, 6, 1, 0xFFFFF0D1), (28, 6, 1, 0xC8D4C09D),
    (29, 6, 2, 0xFFF2E2C2), (31, 6, 1, 0xFF877251), (2, 7, 1, 0xFF947F5D), (3, 7, 1, 0xFFF2E2C2),
    (4, 7, 1, 0xFFFFF0D1), (29, 7, 2, 0xFFF2E2C2), (31, 7, 1, 0xFF877251), (2, 8, 1, 0xFF947F5D),
    (3, 8, 2, 0xFFFFF0D1), (29, 8, 2, 0xFFF2E2C2), (31, 8, 1, 0xFF877251), (2, 9, 1, 0xFF947F5D),
    (3, 9, 1, 0xFFF2E2C2), (4, 9, 1, 0xFFFFF0D1), (29, 9, 2, 0xFFF2E2C2), (31, 9, 1, 0xFF877251),
    (2, 10, 1, 0xFF947F5D), (3, 10, 2, 0xFFF2E2C2), (5, 10, 1, 0xC8D4C09D), (29, 10, 1, 0xFFF2E2C2),
    (30, 10, 1, 0xFFEBD7B2), (31, 10, 1, 0xFF736041), (2, 11, 1, 0xFF947F5D), (3, 11, 2, 0xFFF2E2C2),
    (5, 11, 1, 0xFFD4C09D), (29, 11, 1, 0xFFEBD7B2), (30, 11, 1, 0xFF736041), (2, 12, 1, 0xFF947F5D),
    (3, 12, 1, 0xFFEBD7B2), (4, 12, 1, 0xFFF2E2C2), (5, 12, 1, 0xFFBAA57F), (29, 12, 1, 0xFFD4C09D),
    (30, 12, 1, 0xFF736041), (2, 13, 1, 0xFF877251), (3, 13, 2, 0xFFD4C09D), (5, 13, 1, 0xC8BAA57F),
    (29, 13, 1, 0xFFEBD7B2), (30, 13, 1, 0xFF736041), (1, 14, 1, 0xFF877251), (2, 14, 1, 0xFFF2E2C2),
    (3, 14, 1, 0xFFEBD7B2), (4, 14, 1, 0xC8BAA57F), (29, 14, 1, 0xFFEBD7B2), (30, 14, 1, 0xFF736041),
    (1, 15, 1, 0xFF947F5D), (2, 15, 2, 0xFFF2E2C2), (29, 15, 1, 0xFFEBD7B2), (30, 15, 1, 0xFF736041),
    (1, 16, 1, 0xFF947F5D), (2, 16, 1, 0xFFF2E2C2), (3, 16, 1, 0xFFFFF0D1), (28, 16, 1, 0xC8BAA57F),
    (29, 16, 1, 0xFFEBD7B2), (30, 16, 1, 0xFF736041), (1, 17, 1, 0xFF947F5D), (2, 17, 1, 0xFFFFF0D1),
    (3, 17, 1, 0xC8FFF0D1), (28, 17, 2, 0xFFD4C09D), (30, 17, 1, 0xFF736041), (1, 18, 1, 0xFF947F5D),
    (2, 18, 1, 0xFFFFF0D1), (28, 18, 1, 0xFFD4C09D), (29, 18, 1, 0xFF736041), (1, 19, 1, 0xFF947F5D),
    (2, 19, 1, 0xFFFFF0D1), (27, 19, 1, 0xC8D4C09D), (28, 19, 1, 0xFFEBD7B2), (29, 19, 1, 0xFF736041),
    (1, 20, 1, 0xFF947F5D), (2, 20, 1, 0xFFFFF0D1), (27, 20, 1, 0xFFD4C09D), (28, 20, 1, 0xFFF2E2C2),
    (29, 20, 1, 0xFF736041), (0, 21, 1, 0xFF947F5D), (1, 21, 1, 0xFFF2E2C2), (2, 21, 1, 0xFFFFF0D1),
    (27, 21, 2, 0xFFEBD7B2), (29, 21, 1, 0xFF736041), (0, 22, 1, 0xFF947F5D), (1, 22, 1, 0xFFD4C09D),
    (2, 22, 1, 0xFFF2E2C2), (27, 22, 1, 0xFFEBD7B2), (28, 22, 1, 0xFFF2E2C2), (29, 22, 1, 0xFF736041),
    (0, 23, 1, 0xFF947F5D), (1, 23, 2, 0xFFD4C09D), (27, 23, 2, 0xFFF2E2C2), (29, 23, 1, 0xFF736041),
    (0, 24, 1, 0xFF947F5D), (1, 24, 1, 0xFFEBD7B2), (2, 24, 1, 0xFFD4C09D), (27, 24, 2, 0xFFF2E2C2),
    (29, 24, 1, 0xFF736041), (0, 25, 1, 0xFF877251), (1, 25, 2, 0xFFEBD7B2), (27, 25, 1, 0xFFF2E2C2),
    (28, 25, 1, 0xFF736041), (0, 26, 1, 0xFF877251), (1, 26, 1, 0xFFF2E2C2), (2, 26, 1, 0xFFEBD7B2),
    (3, 26, 1, 0xC8D4C09D), (26, 26, 1, 0xC8D4C09D), (27, 26, 1, 0xFFFFF0D1), (28, 26, 1, 0xFF736041),
    (0, 27, 1, 0xFF877251), (1, 27, 1, 0xFFEBD7B2), (2, 27, 5, 0xFFF2E2C2), (7, 27, 2, 0xFFEBD7B2),
    (9, 27, 1, 0xC8D4C09D), (26, 27, 1, 0xFFF2E2C2), (27, 27, 1, 0xFFFFF0D1), (28, 27, 1, 0xFF736041),
    (0, 28, 5, 0xFF877251), (5, 28, 1, 0xFF736041), (6, 28, 1, 0xFF877251), (7, 28, 3, 0xFFF2E2C2),
    (10, 28, 2, 0xFFEBD7B2), (12, 28, 3, 0xFFD4C09D), (15, 28, 1, 0xC8D4C09D), (25, 28, 1, 0xC8D4C09D),
    (26, 28, 2, 0xFFFFF0D1), (28, 28, 1, 0xFF736041), (7, 29, 1, 0xFF877251), (8, 29, 6, 0xFF736041),
    (14, 29, 1, 0xFFBAA57F), (15, 29, 2, 0xFFD4C09D), (17, 29, 1, 0xFFEBD7B2), (18, 29, 2, 0xFFF2E2C2),
    (20, 29, 7, 0xFFFFF0D1), (27, 29, 1, 0xFFF2E2C2), (28, 29, 1, 0xFF736041), (14, 30, 7, 0xFF736041),
    (21, 30, 1, 0xFFF2E2C2), (22, 30, 3, 0xFFFFF0D1), (25, 30, 3, 0xFFF2E2C2), (28, 30, 1, 0xFF877251),
    (21, 31, 5, 0xFF736041), (26, 31, 3, 0xFF877251),
];

/// Pixel runs for frame 64 (64x64).
pub const FRAME_64: &[(u8, u8, u8, u32)] = &[
    (7, 0, 8, 0xFF947F5D), (7, 1, 1, 0xFF947F5D), (8, 1, 5, 0xFFFFF0D1), (13, 1, 2, 0xFFF2E2C2),
    (15, 1, 7, 0xFF947F5D), (7, 2, 1, 0xFF947F5D), (8, 2, 12, 0xFFFFF0D1), (20, 2, 2, 0xFFF2E2C2),
    (22, 2, 7, 0xFF947F5D), (7, 3, 1, 0xFF947F5D), (8, 3, 19, 0xFFFFF0D1), (27, 3, 2, 0xFFF2E2C2),
    (29, 3, 7, 0xFF947F5D), (7, 4, 1, 0xFF947F5D), (8, 4, 1, 0xFFFFF0D1), (9, 4, 2, 0xFFF2E2C2),
    (11, 4, 20, 0xFFFFF0D1), (31, 4, 4, 0xFFF2E2C2), (35, 4, 1, 0xFFEBD7B2), (36, 4, 7, 0xFF947F5D),
    (7, 5, 1, 0xFF947F5D), (8, 5, 2, 0xFFFFF0D1), (10, 5, 2, 0xFFF2E2C2), (12, 5, 4, 0xFFFFF0D1),
    (16, 5, 1, 0xC8FFF0D1), (26, 5, 1, 0xC8D4C09D), (27, 5, 2, 0xFFEBD7B2), (29, 5, 4, 0xFFF2E2C2),
    (33, 5, 6, 0xFFEBD7B2), (39, 5, 4, 0xFFD4C09D), (43, 5, 7, 0xFF947F5D), (7, 6, 1, 0xFF947F5D),
    (8, 6, 2, 0xFFFFF0D1), (10, 6, 3, 0xFFF2E2C2), (13, 6, 1, 0xC8D4C09D), (34, 6, 1, 0xC8BAA57F),
    (35, 6, 9, 0xFFD4C09D), (44, 6, 6, 0xFFEBD7B2), (50, 6, 7, 0xFF947F5D), (6, 7, 1, 0xFF947F5D),
    (7, 7, 1, 0xFFF2E2C2), (8, 7, 2, 0xFFFFF0D1), (10, 7, 1, 0xFFF2E2C2), (11, 7, 1, 0xC8D4C09D),
    (41, 7, 1, 0xC8BAA57F), (42, 7, 1, 0xFFBAA57F), (43, 7, 4, 0xFFD4C09D), (47, 7, 10, 0xFFEBD7B2),
    (57, 7, 6, 0xFF947F5D), (63, 7, 1, 0xFF877251), (6, 8, 1, 0xFF947F5D), (7, 8, 3, 0xFFFFF0D1),
    (10, 8, 1, 0xFFF2E2C2), (46, 8, 1, 0xC8D4C09D), (47, 8, 2, 0xFFD4C09D), (49, 8, 14, 0xFFEBD7B2),
    (63, 8, 1, 0xFF877251), (6, 9, 1, 0xFF947F5D), (7, 9, 3, 0xFFFFF0D1), (10, 9, 1, 0xC8D4C09D),
    (50, 9, 1, 0xC8D4C09D), (51, 9, 4, 0xFFEBD7B2), (55, 9, 4, 0xFFF2E2C2), (59, 9, 4, 0xFFEBD7B2),
    (63, 9, 1, 0xFF877251), (6, 10, 1, 0xFF947F5D), (7, 10, 3, 0xFFFFF0D1), (53, 10, 1, 0xC8D4C09D),
    (54, 10, 2, 0xFFEBD7B2), (56, 10, 5, 0xFFF2E2C2), (61, 10, 2, 0xFFEBD7B2), (63, 10, 1, 0xFF877251),
    (6, 11, 1, 0xFF947F5D), (7, 11, 3, 0xFFFFF0D1), (55, 11, 1, 0xC8D4C09D), (56, 11, 1, 0xFFEBD7B2),
    (57, 11, 5, 0xFFF2E2C2), (62, 11, 1, 0xFFEBD7B2), (63, 11, 1, 0xFF877251), (6, 12, 1, 0xFF947F5D),
    (7, 12, 3, 0xFFFFF0D1), (56, 12, 1, 0xC8D4C09D), (57, 12, 6, 0xFFF2E2C2), (63, 12, 1, 0xFF877251),
    (6, 13, 1, 0xFF947F5D), (7, 13, 3, 0xFFFFF0D1), (57, 13, 6, 0xFFF2E2C2), (63, 13, 1, 0xFF877251),
    (5, 14, 1, 0xFF947F5D), (6, 14, 1, 0xFFF2E2C2), (7, 14, 3, 0xFFFFF0D1), (57, 14, 1, 0xC8D4C09D),
    (58, 14, 4, 0xFFF2E2C2), (62, 14, 1, 0xFFEBD7B2), (63, 14, 1, 0xFF877251), (5, 15, 1, 0xFF947F5D),
    (6, 15, 4, 0xFFFFF0D1), (58, 15, 4, 0xFFF2E2C2), (62, 15, 1, 0xFF877251), (5, 16, 1, 0xFF947F5D),
    (6, 16, 3, 0xFFFFF0D1), (9, 16, 1, 0xFFF2E2C2), (58, 16, 1, 0xFFEBD7B2), (59, 16, 3, 0xFFF2E2C2),
    (62, 16, 1, 0xFF877251), (5, 17, 1, 0xFF947F5D), (6, 17, 3, 0xFFFFF0D1), (9, 17, 1, 0xFFF2E2C2),
    (58, 17, 1, 0xC8D4C09D), (59, 17, 3, 0xFFF2E2C2), (62, 17, 1, 0xFF877251), (5, 18, 1, 0xFF947F5D),
    (6, 18, 2, 0xFFFFF0D1), (8, 18, 2, 0xFFF2E2C2), (59, 18, 2, 0xFFF2E2C2), (61, 18, 1, 0xFFEBD7B2),
    (62, 18, 1, 0xFF877251), (5, 19, 1, 0xFF947F5D), (6, 19, 1, 0xFFF2E2C2), (7, 19, 1, 0xFFFFF0D1),
    (8, 19, 2, 0xFFF2E2C2), (10, 19, 1, 0xC8D4C09D), (59, 19, 1, 0xFFEBD7B2), (60, 19, 1, 0xFFF2E2C2),
    (61, 19, 1, 0xFFEBD7B2), (62, 19, 1, 0xFF736041), (5, 20, 1, 0xFF947F5D), (6, 20, 4, 0xFFF2E2C2),
    (10, 20, 1, 0xFFEBD7B2), (59, 20, 3, 0xFFEBD7B2), (62, 20, 1, 0xFF736041), (4, 21, 1, 0xFF947F5D),
    (5, 21, 6, 0xFFF2E2C2), (11, 21, 1, 0xC8D4C09D), (59, 21, 3, 0xFFEBD7B2), (62, 21, 1, 0xFF736041),
    (4, 22, 1, 0xFF947F5D), (5, 22, 5, 0xFFF2E2C2), (10, 22, 1, 0xFFEBD7B2), (11, 22, 1, 0xFFD4C09D),
    (59, 22, 2, 0xFFEBD7B2), (61, 22, 1, 0xFF736041), (4, 23, 1, 0xFF947F5D), (5, 23, 1, 0xFFEBD7B2),
    (6, 23, 4, 0xFFF2E2C2), (10, 23, 1, 0xFFEBD7B2), (11, 23, 1, 0xFFBAA57F), (59, 23, 1, 0xFFD4C09D),
    (60, 23, 1, 0xFFEBD7B2), (61, 23, 1, 0xFF736041), (4, 24, 1, 0xFF947F5D), (5, 24, 1, 0xFFEBD7B2),
    (6, 24, 4, 0xFFF2E2C2), (10, 24, 1, 0xFFD4C09D), (11, 24, 1, 0xC8BAA57F), (59, 24, 1, 0xFFD4C09D),
    (60, 24, 1, 0xFFEBD7B2), (61, 24, 1, 0xFF736041), (4, 25, 1, 0xFF947F5D), (5, 25, 1, 0xFFD4C09D),
    (6, 25, 1, 0xFFEBD7B2), (7, 25, 2, 0xFFF2E2C2), (9, 25, 1, 0xFFEBD7B2), (10, 25, 1, 0xFFD4C09D),
    (59, 25, 2, 0xFFD4C09D), (61, 25, 1, 0xFF736041), (4, 26, 1, 0xFF877251), (5, 26, 1, 0xFFEBD7B2),
    (6, 26, 2, 0xFFD4C09D), (8, 26, 1, 0xFFEBD7B2), (9, 26, 1, 0xFFD4C09D), (10, 26, 1, 0xFFBAA57F),
    (59, 26, 2, 0xFFD4C09D), (61, 26, 1, 0xFF736041), (4, 27, 1, 0xFF877251), (5, 27, 1, 0xFFF2E2C2),
    (6, 27, 1, 0xFFEBD7B2), (7, 27, 3, 0xFFD4C09D), (10, 27, 1, 0xC8BAA57F), (58, 27, 1, 0xC8BAA57F),
    (59, 27, 2, 0xFFD4C09D), (61, 27, 1, 0xFF736041), (3, 28, 1, 0xFF877251), (4, 28, 3, 0xFFF2E2C2),
    (7, 28, 1, 0xFFEBD7B2), (8, 28, 1, 0xFFD4C09D), (9, 28, 1, 0xC8BAA57F), (58, 28, 1, 0xFFD4C09D),
    (59, 28, 1, 0xFFEBD7B2), (60, 28, 1, 0xFFD4C09D), (61, 28, 1, 0xFF736041), (3, 29, 1, 0xFF947F5D),
    (4, 29, 1, 0xFFF2E2C2), (5, 29, 1, 0xFFFFF0D1), (6, 29, 2, 0xFFF2E2C2), (8, 29, 1, 0xFFD4C09D),
    (58, 29, 1, 0xFFD4C09D), (59, 29, 1, 0xFFEBD7B2), (60, 29, 1, 0xFF736041), (3, 30, 1, 0xFF947F5D),
    (4, 30, 1, 0xFFF2E2C2), (5, 30, 2, 0xFFFFF0D1), (7, 30, 1, 0xFFF2E2C2), (8, 30, 1, 0xC8BAA57F),
    (58, 30, 1, 0xFFEBD7B2), (59, 30, 1, 0xFFF2E2C2), (60, 30, 1, 0xFF736041), (3, 31, 1, 0xFF947F5D),
    (4, 31, 1, 0xFFF2E2C2), (5, 31, 2, 0xFFFFF0D1), (7, 31, 1, 0xFFF2E2C2), (58, 31, 1, 0xFFEBD7B2),
    (59, 31, 1, 0xFFF2E2C2), (60, 31, 1, 0xFF736041), (3, 32, 1, 0xFF947F5D), (4, 32, 3, 0xFFFFF0D1),
    (7, 32, 1, 0xFFEBD7B2), (57, 32, 1, 0xC8BAA57F), (58, 32, 1, 0xFFEBD7B2), (59, 32, 1, 0xFFF2E2C2),
    (60, 32, 1, 0xFF736041), (3, 33, 1, 0xFF947F5D), (4, 33, 3, 0xFFFFF0D1), (7, 33, 1, 0xC8EBD7B2),
    (57, 33, 1, 0xFFD4C09D), (58, 33, 2, 0xFFEBD7B2), (60, 33, 1, 0xFF736041), (3, 34, 1, 0xFF947F5D),
    (4, 34, 3, 0xFFFFF0D1), (57, 34, 1, 0xFFD4C09D), (58, 34, 2, 0xFFEBD7B2), (60, 34, 1, 0xFF736041),
    (2, 35, 1, 0xFF947F5D), (3, 35, 1, 0xFFF2E2C2), (4, 35, 2, 0xFFFFF0D1), (6, 35, 1, 0xC8F2E2C2),
    (56, 35, 1, 0xC8BAA57F), (57, 35, 1, 0xFFD4C09D), (58, 35, 1, 0xFFEBD7B2), (59, 35, 1, 0xFFD4C09D),
    (60, 35, 1, 0xFF736041), (2, 36, 1, 0xFF947F5D), (3, 36, 3, 0xFFFFF0D1), (56, 36, 3, 0xFFD4C09D),
    (59, 36, 1, 0xFF736041), (2, 37, 1, 0xFF947F5D), (3, 37, 3, 0xFFFFF0D1), (56, 37, 2, 0xFFD4C09D),
    (58, 37, 1, 0xFFEBD7B2), (59, 37, 1, 0xFF736041), (2, 38, 1, 0xFF947F5D), (3, 38, 2, 0xFFFFF0D1),
    (5, 38, 1, 0xFFF2E2C2), (55, 38, 1, 0xC8D4C09D), (56, 38, 1, 0xFFD4C09D), (57, 38, 2, 0xFFEBD7B2),
    (59, 38, 1, 0xFF736041), (2, 39, 1, 0xFF947F5D), (3, 39, 2, 0xFFFFF0D1), (5, 39, 1, 0xC8F2E2C2),
    (55, 39, 1, 0xFFD4C09D), (56, 39, 3, 0xFFEBD7B2), (59, 39, 1, 0xFF736041), (2, 40, 1, 0xFF947F5D),
    (3, 40, 2, 0xFFFFF0D1), (55, 40, 2, 0xFFEBD7B2), (57, 40, 1, 0xFFF2E2C2), (58, 40, 1, 0xFFEBD7B2),
    (59, 40, 1, 0xFF736041), (2, 41, 1, 0xFF947F5D), (3, 41, 2, 0xFFFFF0D1), (55, 41, 1, 0xFFEBD7B2),
    (56, 41, 2, 0xFFF2E2C2), (58, 41, 1, 0xFFEBD7B2), (59, 41, 1, 0xFF736041), (1, 42, 1, 0xFF947F5D),
    (2, 42, 1, 0xFFF2E2C2), (3, 42, 2, 0xFFFFF0D1), (55, 42, 1, 0xFFEBD7B2), (56, 42, 2, 0xFFF2E2C2),
    (58, 42, 1, 0xFFEBD7B2), (59, 42, 1, 0xFF736041), (1, 43, 1, 0xFF947F5D), (2, 43, 1, 0xFFEBD7B2),
    (3, 43, 2, 0xFFFFF0D1), (55, 43, 1, 0xFFEBD7B2), (56, 43, 1, 0xFFF2E2C2), (57, 43, 1, 0xFFEBD7B2),
    (58, 43, 1, 0xFF736041), (1, 44, 1, 0xFF947F5D), (2, 44, 1, 0xFFEBD7B2), (3, 44, 1, 0xFFF2E2C2),
    (4, 44, 1, 0xFFFFF0D1), (55, 44, 3, 0xFFEBD7B2), (58, 44, 1, 0xFF736041), (1, 45, 1, 0xFF947F5D),
    (2, 45, 1, 0xFFD4C09D), (3, 45, 1, 0xFFEBD7B2), (4, 45, 1, 0xFFF2E2C2), (55, 45, 1, 0xFFEBD7B2),
    (56, 45, 1, 0xFFF2E2C2), (57, 45, 1, 0xFFEBD7B2), (58, 45, 1, 0xFF736041), (1, 46, 1, 0xFF947F5D),
    (2, 46, 2, 0xFFD4C09D), (4, 46, 1, 0xFFEBD7B2), (54, 46, 1, 0xC8D4C09D), (55, 46, 1, 0xFFEBD7B2),
    (56, 46, 2, 0xFFF2E2C2), (58, 46, 1, 0xFF736041), (1, 47, 1, 0xFF947F5D), (2, 47, 1, 0xFFEBD7B2),
    (3, 47, 2, 0xFFD4C09D), (54, 47, 1, 0xFFD4C09D), (55, 47, 3, 0xFFF2E2C2), (58, 47, 1, 0xFF736041),
    (1, 48, 1, 0xFF947F5D), (2, 48, 2, 0xFFEBD7B2), (4, 48, 1, 0xFFD4C09D), (54, 48, 1, 0xFFEBD7B2),
    (55, 48, 3, 0xFFF2E2C2), (58, 48, 1, 0xFF736041), (0, 49, 1, 0xFF947F5D), (1, 49, 1, 0xFFEBD7B2),
    (2, 49, 1, 0xFFF2E2C2), (3, 49, 1, 0xFFEBD7B2), (4, 49, 1, 0xFFD4C09D), (54, 49, 1, 0xFFEBD7B2),
    (55, 49, 2, 0xFFF2E2C2), (57, 49, 1, 0xFFEBD7B2), (58, 49, 1, 0xFF736041), (0, 50, 1, 0xFF877251),
    (1, 50, 2, 0xFFF2E2C2), (3, 50, 2, 0xFFEBD7B2), (5, 50, 1, 0xC8D4C09D), (54, 50, 3, 0xFFF2E2C2),
    (57, 50, 1, 0xFF736041), (0, 51, 1, 0xFF877251), (1, 51, 3, 0xFFF2E2C2), (4, 51, 2, 0xFFEBD7B2),
    (54, 51, 3, 0xFFF2E2C2), (57, 51, 1, 0xFF736041), (0, 52, 1, 0xFF877251), (1, 52, 4, 0xFFF2E2C2),
    (5, 52, 1, 0xFFEBD7B2), (6, 52, 1, 0xC8D4C09D), (53, 52, 1, 0xC8D4C09D), (54, 52, 3, 0xFFF2E2C2),
    (57, 52, 1, 0xFF736041), (0, 53, 1, 0xFF877251), (1, 53, 1, 0xFFEBD7B2), (2, 53, 5, 0xFFF2E2C2),
    (7, 53, 1, 0xFFEBD7B2), (8, 53, 1, 0xC8D4C09D), (53, 53, 1, 0xFFF2E2C2), (54, 53, 1, 0xFFFFF0D1),
    (55, 53, 2, 0xFFF2E2C2), (57, 53, 1, 0xFF736041), (0, 54, 1, 0xFF877251), (1, 54, 3, 0xFFEBD7B2),
    (4, 54, 7, 0xFFF2E2C2), (11, 54, 1, 0xFFEBD7B2), (12, 54, 1, 0xC8EBD7B2), (52, 54, 1, 0xC8D4C09D),
    (53, 54, 2, 0xFFFFF0D1), (55, 54, 2, 0xFFF2E2C2), (57, 54, 1, 0xFF736041), (0, 55, 1, 0xFF877251),
    (1, 55, 5, 0xFFEBD7B2), (6, 55, 9, 0xFFF2E2C2), (15, 55, 2, 0xFFEBD7B2), (17, 55, 2, 0xFFD4C09D),
    (19, 55, 1, 0xC8D4C09D), (52, 55, 1, 0xFFF2E2C2), (53, 55, 3, 0xFFFFF0D1), (56, 55, 1, 0xFFF2E2C2),
    (57, 55, 1, 0xFF736041), (0, 56, 7, 0xFF877251), (7, 56, 2, 0xFFEBD7B2), (9, 56, 8, 0xFFF2E2C2),
    (17, 56, 4, 0xFFEBD7B2), (21, 56, 4, 0xFFD4C09D), (25, 56, 1, 0xC8D4C09D), (51, 56, 1, 0xC8D4C09D),
    (52, 56, 4, 0xFFFFF0D1), (56, 56, 1, 0xFFF2E2C2), (57, 56, 1, 0xFF736041), (7, 57, 5, 0xFF877251),
    (12, 57, 1, 0xFF736041), (13, 57, 1, 0xFF877251), (14, 57, 2, 0xFFD4C09D), (16, 57, 3, 0xFFEBD7B2),
    (19, 57, 12, 0xFFD4C09D), (31, 57, 1, 0xC8D4C09D), (50, 57, 1, 0xC8D4C09D), (51, 57, 5, 0xFFFFF0D1),
    (56, 57, 1, 0xFF736041), (14, 58, 1, 0xFF877251), (15, 58, 6, 0xFF736041), (21, 58, 1, 0xFFBAA57F),
    (22, 58, 7, 0xFFD4C09D), (29, 58, 4, 0xFFEBD7B2), (33, 58, 4, 0xFFF2E2C2), (37, 58, 1, 0xC8F2E2C2),
    (48, 58, 1, 0xC8D4C09D), (49, 58, 1, 0xFFF2E2C2), (50, 58, 5, 0xFFFFF0D1), (55, 58, 1, 0xFFF2E2C2),
    (56, 58, 1, 0xFF736041), (21, 59, 7, 0xFF736041), (28, 59, 2, 0xFFBAA57F), (30, 59, 1, 0xFFD4C09D),
    (31, 59, 1, 0xFFEBD7B2), (32, 59, 3, 0xFFF2E2C2), (35, 59, 19, 0xFFFFF0D1), (54, 59, 2, 0xFFF2E2C2),
    (56, 59, 1, 0xFF736041), (28, 60, 7, 0xFF736041), (35, 60, 2, 0xFFF2E2C2), (37, 60, 16, 0xFFFFF0D1),
    (53, 60, 3, 0xFFF2E2C2), (56, 60, 1, 0xFF736041), (35, 61, 7, 0xFF736041), (42, 61, 2, 0xFFF2E2C2),
    (44, 61, 7, 0xFFFFF0D1), (51, 61, 5, 0xFFF2E2C2), (56, 61, 1, 0xFF877251), (42, 62, 7, 0xFF736041),
    (49, 62, 7, 0xFFF2E2C2), (56, 62, 1, 0xFF877251), (49, 63, 4, 0xFF736041), (53, 63, 4, 0xFF877251),
];

/// Pixel runs for locked glass (16x16).
pub const LOCKED_GLASS: &[(u8, u8, u8, u32)] = &[
    (1, 0, 3, 0xFF5D8FC2), (4, 0, 4, 0xFF8BADD0), (8, 0, 1, 0xFF5D8FC2), (1, 1, 1, 0xFF5D8FC2),
    (9, 1, 2, 0xFF5D8FC2), (11, 1, 3, 0xFF8BADD0), (14, 1, 2, 0xFF5D8FC2), (1, 2, 1, 0xFF8BADD0),
    (7, 2, 3, 0xFFD4E5F7), (10, 2, 1, 0xFFB3CFEC), (15, 2, 1, 0xFF5D8FC2), (1, 3, 1, 0xFF8BADD0),
    (5, 3, 3, 0xFFD4E5F7), (8, 3, 1, 0xFFB3CFEC), (15, 3, 1, 0xFF5D8FC2), (1, 4, 1, 0xFF8BADD0),
    (3, 4, 3, 0xFFD4E5F7), (6, 4, 1, 0xFFB3CFEC), (15, 4, 1, 0xFF5D8FC2), (1, 5, 1, 0xFF5D8FC2),
    (2, 5, 2, 0xFFD4E5F7), (4, 5, 1, 0xFFB3CFEC), (15, 5, 1, 0xFF5D8FC2), (1, 6, 1, 0xFF5D8FC2),
    (2, 6, 1, 0xFFB3CFEC), (15, 6, 1, 0xFF5D8FC2), (0, 7, 1, 0xFF5D8FC2), (15, 7, 1, 0xFF4F79A8),
    (0, 8, 1, 0xFF5D8FC2), (15, 8, 1, 0xFF4F79A8), (0, 9, 1, 0xFF5D8FC2), (14, 9, 1, 0xFF4F79A8),
    (0, 10, 1, 0xFF5D8FC2), (14, 10, 1, 0xFF4F79A8), (0, 11, 1, 0xFF5D8FC2), (14, 11, 1, 0xFF4F79A8),
    (0, 12, 1, 0xFF5D8FC2), (14, 12, 1, 0xFF4F79A8), (0, 13, 1, 0xFF4F79A8), (14, 13, 1, 0xFF4F79A8),
    (0, 14, 4, 0xFF3B608A), (4, 14, 3, 0xFF4F79A8), (14, 14, 1, 0xFF3B608A), (7, 15, 2, 0xFF4F79A8),
    (9, 15, 2, 0xFF3B608A), (11, 15, 1, 0xFF4F79A8), (12, 15, 3, 0xFF3B608A),
];

/// Pixel runs for locked glass full (16x16).
pub const LOCKED_GLASS_FULL: &[(u8, u8, u8, u32)] = &[
    (0, 0, 1, 0xFFD4E5F7), (1, 0, 7, 0xFF8BADD0), (8, 0, 8, 0xFF5D8FC2), (0, 1, 1, 0xFF8BADD0),
    (15, 1, 1, 0xFF4F79A8), (0, 2, 1, 0xFF8BADD0), (7, 2, 1, 0xFFB3CFEC), (15, 2, 1, 0xFF4F79A8),
    (0, 3, 1, 0xFF8BADD0), (6, 3, 1, 0xFFD4E5F7), (7, 3, 1, 0xFFB3CFEC), (15, 3, 1, 0xFF4F79A8),
    (0, 4, 1, 0xFF8BADD0), (4, 4, 1, 0xFFB3CFEC), (5, 4, 2, 0xFFD4E5F7), (15, 4, 1, 0xFF4F79A8),
    (0, 5, 1, 0xFF8BADD0), (3, 5, 1, 0xFFB3CFEC), (4, 5, 2, 0xFFD4E5F7), (15, 5, 1, 0xFF4F79A8),
    (0, 6, 1, 0xFF8BADD0), (3, 6, 1, 0xFFD4E5F7), (4, 6, 1, 0xFFB3CFEC), (15, 6, 1, 0xFF4F79A8),
    (0, 7, 1, 0xFF8BADD0), (2, 7, 1, 0xFFB3CFEC), (15, 7, 1, 0xFF4F79A8), (0, 8, 1, 0xFF5D8FC2),
    (15, 8, 1, 0xFF4F79A8), (0, 9, 1, 0xFF5D8FC2), (15, 9, 1, 0xFF4F79A8), (0, 10, 1, 0xFF5D8FC2),
    (15, 10, 1, 0xFF4F79A8), (0, 11, 1, 0xFF5D8FC2), (15, 11, 1, 0xFF4F79A8), (0, 12, 1, 0xFF5D8FC2),
    (15, 12, 1, 0xFF4F79A8), (0, 13, 1, 0xFF5D8FC2), (15, 13, 1, 0xFF4F79A8), (0, 14, 1, 0xFF5D8FC2),
    (15, 14, 1, 0xFF4F79A8), (0, 15, 1, 0xFF4F79A8), (1, 15, 15, 0xFF3B608A),
];

/// The frame pixel runs and scaling step for an item atlas resolution.
pub fn frame_runs(res: u32) -> (&'static [(u8, u8, u8, u32)], f32) {
    if res >= 64 {
        (FRAME_64, 0.25)
    } else if res >= 32 {
        (FRAME_32, 0.5)
    } else {
        (FRAME_16, 1.0)
    }
}

/// Turns pixel runs into filled GUI rectangles at `[at[0], at[1]]` with `step`.
pub fn runs_to_rects(runs: &[(u8, u8, u8, u32)], at: [f32; 2], step: f32) -> Vec<Rect> {
    runs.iter()
        .map(|&(x, y, len, c)| {
            ([at[0] + x as f32 * step, at[1] + y as f32 * step, len as f32 * step, step], c)
        })
        .collect()
}

#[derive(Default)]
struct State {
    settings: Option<Settings>,
    shift: [bool; 2],
    /// The map whose tooltip space was asked for this frame, and its size:
    /// the draw hook only draws what this mod measured.
    claimed: Option<(i32, [f32; 2])>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The settings, read once through `get`, then kept.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    STATE.with_borrow_mut(|s| *s.settings.get_or_insert_with(|| Settings::read(get)))
}

/// Replaces the settings (the settings page changed them).
pub fn set_settings(settings: Settings) {
    STATE.with_borrow_mut(|s| s.settings = Some(settings));
}

/// A key went down or up: Shift is followed for "Hold Shift".
pub fn key(key: &str, pressed: bool) {
    let side = match key {
        "key.keyboard.left.shift" => 0,
        "key.keyboard.right.shift" => 1,
        _ => return,
    };
    STATE.with_borrow_mut(|s| s.shift[side] = pressed);
}

pub fn shift() -> bool {
    STATE.with_borrow(|s| s.shift[0] || s.shift[1])
}

/// The tooltip space this mod asked for map `id`.
pub fn claim(id: i32, size: [f32; 2]) {
    STATE.with_borrow_mut(|s| s.claimed = Some((id, size)));
}

/// Whether the tooltip space `size` for map `id` is this mod's to draw in.
pub fn claimed(id: i32, size: [f32; 2]) -> bool {
    STATE.with_borrow(|s| s.claimed == Some((id, size)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALETTE: [u32; 4] = [0, 0x7F_B238, 0x40_40FF, 0xFF_FFFF];

    fn stored<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        |key| pairs.iter().find(|p| p.0 == key).map(|p| p.1.to_owned())
    }

    #[test]
    fn options_have_their_defaults_among_their_choices() {
        for opt in &OPTS {
            assert!(opt.choices.iter().any(|c| c.0 == opt.default), "{}", opt.key);
        }
    }

    #[test]
    fn settings_read_click_and_reset() {
        assert_eq!(Settings::read(stored(&[])), Settings::default());
        let mut s = Settings::read(stored(&[
            ("tooltip", "2"),
            ("tooltip_size", "128
"),
            ("paper", "1"),
            ("slot", "7"),
            ("slot_style", "2"),
            ("slot_res", "1"),
            ("locked_glass", "0"),
        ]));
        assert_eq!(s.tooltip_side(false), None);
        assert_eq!(s.tooltip_side(true), Some(128.0));
        assert_eq!(s.get(o::SLOT), 1, "not a choice: the default");
        assert_eq!(s.get(o::PAPER), 1);
        assert_eq!(s.get(o::SLOT_STYLE), 2);
        assert_eq!(s.get(o::SLOT_RES), 1);
        assert_eq!(s.get(o::LOCKED_GLASS), 0);
        assert_eq!(s.click(o::TOOLTIP), [("tooltip", "0".to_owned())]);
        assert_eq!(s.tooltip_side(true), None);
        assert_eq!(s.labels()[o::TOOLTIP], "Tooltip preview: OFF");
        assert_eq!(s.labels()[OPTS.len()], RESET_LABEL);
        assert_eq!(s.click(OPTS.len()).len(), OPTS.len());
        assert_eq!(s, Settings::default());
        assert_eq!(s.tooltip_side(false), Some(96.0));
        assert!(s.click(42).is_empty());
    }

    #[test]
    fn packed_colours_follow_vanilla_shades() {
        assert_eq!(argb(0, &PALETTE), None);
        assert_eq!(argb(3, &PALETTE), None, "id 0 is empty whatever its shade");
        // Grass, brightest shade (255): its own colour.
        assert_eq!(argb(1 << 2 | 2, &PALETTE), Some(0xFF7F_B238));
        // White at shade 0 (180/255).
        assert_eq!(argb(3 << 2, &PALETTE), Some(0xFFB4_B4B4));
        assert_eq!(argb(9 << 2, &PALETTE), None, "past the palette");
    }

    fn map(f: impl Fn(usize, usize) -> u8) -> Vec<u8> {
        (0..MAP_SIZE * MAP_SIZE).map(|i| f(i % MAP_SIZE, i / MAP_SIZE)).collect()
    }

    #[test]
    fn picture_merges_runs_and_skips_empty() {
        // Left half grass, right half water.
        let colors = map(|x, _| if x < 64 { 1 << 2 | 2 } else { 2 << 2 | 2 });
        let rects = picture(&colors, &PALETTE, [10.0, 20.0], 64.0, 64);
        assert_eq!(rects.len(), 128, "two runs a row");
        assert_eq!(rects[0], ([10.0, 20.0, 32.0, 1.0], 0xFF7F_B238));
        assert_eq!(rects[1], ([42.0, 20.0, 32.0, 1.0], 0xFF40_40FF));
        // An empty map: nothing but the paper.
        assert!(picture(&map(|_, _| 0), &PALETTE, [0.0; 2], 64.0, 64).is_empty());
        assert!(picture(&[1, 2, 3], &PALETTE, [0.0; 2], 64.0, 64).is_empty());
    }

    #[test]
    fn tooltip_and_slot_layouts() {
        // Left half grass, right half empty (so paper background shows through on right half)
        let colors = map(|x, _| if x < 64 { 1 << 2 | 2 } else { 0 });

        // Tooltip with paper background (bg = 1)
        let t_paper = tooltip_rects(&colors, &PALETTE, [5.0, 5.0], 96.0, 1);
        assert_eq!(t_paper[0], ([5.0, 7.0, 96.0, 96.0], PAPER));
        assert_eq!(t_paper[1], ([5.0, 7.0, 48.0, 1.0], 0xFF7F_B238));

        // Tooltip with framed background (bg = 2)
        let t_framed = tooltip_rects(&colors, &PALETTE, [5.0, 5.0], 96.0, 2);
        assert_eq!(tooltip_pad(96.0), 5.0);
        assert_eq!(t_framed[0], ([5.0, 7.0, 106.0, 106.0], PAPER));
        assert_eq!(t_framed[1], ([10.0, 12.0, 48.0, 1.0], 0xFF7F_B238));

        // Tooltip without background (bg = 0)
        let t_none = tooltip_rects(&colors, &PALETTE, [5.0, 5.0], 96.0, 0);
        assert_eq!(t_none[0], ([5.0, 7.0, 48.0, 1.0], 0xFF7F_B238));
        assert_eq!(tooltip_space(96.0, false), [96.0, 98.0]);
        assert_eq!(tooltip_space(96.0, true), [106.0, 108.0]);

        // Slot with border (edged = true)
        let s_edge = slot_rects(&colors, &PALETTE, [0.0, 0.0], true, false, false);
        assert_eq!(s_edge[0], ([0.0, 0.0, 16.0, 16.0], PAPER_EDGE));
        assert_eq!(s_edge[1], ([1.0, 1.0, 14.0, 14.0], PAPER));
        assert_eq!(s_edge[2], ([1.0, 1.0, 7.0, 1.0], 0xFF7F_B238));

        // Slot full slot (edged = false)
        let full = slot_rects(&colors, &PALETTE, [0.0, 0.0], false, false, false);
        assert_eq!(full[0], ([0.0, 0.0, 8.0, 1.0], 0xFF7F_B238));

        // Slot high-res (cells = 128)
        let high = slot_rects(&colors, &PALETTE, [0.0, 0.0], false, false, true);
        assert_eq!(high.len(), 128);
    }

    #[test]
    fn frame_and_glass_runs() {
        let (f16, step16) = frame_runs(16);
        assert_eq!(step16, 1.0);
        assert_eq!(f16.len(), 85);

        let (f32, step32) = frame_runs(32);
        assert_eq!(step32, 0.5);
        assert_eq!(f32.len(), 174);

        let (f64, step64) = frame_runs(64);
        assert_eq!(step64, 0.25);
        assert_eq!(f64.len(), 432);

        assert_eq!(LOCKED_GLASS.len(), 47);
        assert_eq!(LOCKED_GLASS_FULL.len(), 43);

        let rects = runs_to_rects(f16, [10.0, 20.0], 1.0);
        assert_eq!(rects[0], ([11.0, 20.0, 8.0, 1.0], 0xFF947F5D));
    }

    #[test]
    fn tilted_geometry() {
        let paper = tilted_paper_quad([10.0, 20.0]);
        assert_eq!(paper.1, PAPER);
        assert_eq!(paper.0.len(), 4);

        let colors = map(|_, _| 1 << 2 | 2);
        let quads = tilted_picture(&colors, &PALETTE, [10.0, 20.0], 13);
        assert_eq!(quads.len(), 13);
        assert_eq!(quads[0].1, 0xFF7F_B238);
    }

    #[test]
    fn shift_and_claims() {
        key("key.keyboard.left.shift", true);
        assert!(shift());
        key("key.keyboard.left.shift", false);
        key("key.keyboard.a", true);
        assert!(!shift());
        claim(7, [96.0, 98.0]);
        assert!(claimed(7, [96.0, 98.0]));
        assert!(!claimed(8, [96.0, 98.0]));
        assert!(!claimed(7, [64.0, 66.0]));
    }
}
