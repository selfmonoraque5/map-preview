// Map Preview for Horizon: a filled map's picture in its tooltip, and a small
// copy of it in its inventory slot instead of the plain map icon. Inspired by
// the Map Tooltip and Map in Slot mods, written from scratch on Horizon's mod
// API (none of their code). This module is the mod's logic and its tests; the
// hooks and the map item's Part are in lib.rs, and the game patch that hands
// over the maps' pictures in mixins/maps.rs (it reaches this crate as
// `mod_map_preview`).
//
// The pictures come from the maps this client already has (the ones the
// server sent, as for any map you carry); a map the client hasn't been sent
// shows no preview. They are drawn as filled rectangles, one per run of
// same-coloured pixels.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

/// The mod's id (its settings folder); the version is in horizon-mod.toml.
pub const ID: &str = "map_preview";

/// A map's side, in pixels.
pub const MAP_SIZE: usize = 128;
/// Cells per side of a "High" slot picture: a 16 px slot is 64 screen pixels
/// at GUI scale 4, so more cells than this draw nothing finer, only slower.
pub const SLOT_HIGH_CELLS: usize = 64;
/// The map paper behind the picture (vanilla's map background).
pub const PAPER: u32 = 0xFF_DCC9A0;
/// The paper's darker edge.
pub const PAPER_EDGE: u32 = 0xFF_9C8459;
/// Space above the tooltip picture, in GUI pixels.
pub const TOOLTIP_GAP: f32 = 2.0;
/// A slot's icon, in GUI pixels.
pub const SLOT_SIZE: f32 = 16.0;
/// Map rotation in inventory slot as a map item: +8 degrees around the slot centre.
pub const MAP_ITEM_ROTATION_RAD: f32 = 8.0 * std::f32::consts::PI / 180.0;
/// The map picture side inside the map item cutout overlay, in GUI pixels.
pub const MAP_ITEM_INNER_SIZE: f32 = 13.0;

/// One option of the settings page: its file, its English label, the values the player
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

    /// From the settings page, where each option is a choice: `index` gives
    /// the option the player picked for a key (its position in `choices`).
    pub fn from_choices(index: impl Fn(&str) -> Option<u32>) -> Self {
        Self::read(|key| {
            let opt = OPTS.iter().find(|opt| opt.key == key)?;
            let at = usize::try_from(index(key)?).ok()?;
            opt.choices.get(at).map(|c| c.0.to_string())
        })
    }

    /// Option `i`'s value as the page keeps it: its position in `choices`.
    pub fn choice_index(&self, i: usize) -> usize {
        OPTS[i].choices.iter().position(|c| c.0 == self.values[i]).unwrap_or(0)
    }
}

/// The translation key of option `key`'s label, and of its choice `value`.
pub fn label_key(key: &str) -> String {
    format!("{ID}.option.{key}")
}

pub fn choice_key(key: &str, value: u16) -> String {
    format!("{ID}.option.{key}.{value}")
}

/// `assets/map_preview/lang/en_us.json`: every label the page shows, in English.
#[cfg(test)]
pub fn english() -> Vec<(String, String)> {
    let mut lines = Vec::new();
    for opt in &OPTS {
        lines.push((label_key(opt.key), opt.label.to_owned()));
        for (value, name) in opt.choices {
            lines.push((choice_key(opt.key, *value), (*name).to_owned()));
        }
    }
    lines
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
/// One filled frame pixel run: `(x, y, len, argb)`.
pub type FrameRun = (u16, u16, u16, u32);

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

/// Outer padding around the map for framed tooltip background (`bg == 2`).
/// Follows vanilla's map background frame margin (~7 units per 128).
pub fn tooltip_pad(side: f32) -> f32 {
    (side * 7.0 / 128.0).round().max(2.0)
}

/// The tooltip space for a map, `[width, height]`.
/// When `framed` (`bg == 2`), includes padding around the map on all sides.
pub fn tooltip_space(side: f32, framed: bool) -> [f32; 2] {
    if framed {
        let total = side + 2.0 * tooltip_pad(side);
        [total, total + TOOLTIP_GAP]
    } else {
        [side, side + TOOLTIP_GAP]
    }
}

/// The tooltip's picture: background (`0`: none, `1`: paper, `2`: framed paper
/// fallback), then the map.
#[allow(dead_code)]
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
    // One cell per GUI pixel at most: a map pixel smaller than that is lost anyway.
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
    let cells = if high_res { SLOT_HIGH_CELLS } else { side as usize };
    rects.extend(picture(colors, palette, inner, side, cells));
    rects
}

/// Rotates a 2D point `[px, py]` around `center` by an angle with `sin` and `cos`.
#[inline]
pub fn rotate_point(pt: [f32; 2], center: [f32; 2], sin: f32, cos: f32) -> [f32; 2] {
    let dx = pt[0] - center[0];
    let dy = pt[1] - center[1];
    [center[0] + dx * cos - dy * sin, center[1] + dx * sin + dy * cos]
}

/// The paper backing quad tilted for "As map item".
pub fn tilted_paper_quad(at: [f32; 2]) -> Quad {
    let center = [at[0] + SLOT_SIZE / 2.0, at[1] + SLOT_SIZE / 2.0];
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

/// The map's picture tilted for "As map item", sampled to `cells` x `cells`.
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
    let center = [at[0] + SLOT_SIZE / 2.0, at[1] + SLOT_SIZE / 2.0];
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

/// Expands `kept` from `same` (untouched border pixels) across 8-connected
/// `candidate` parchment pixels so isolated interior pixels are discarded.
fn propagate_kept(same: &[bool], candidate: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut kept = same.to_vec();
    let mut queue = VecDeque::new();
    for (idx, &is_same) in same.iter().enumerate() {
        if is_same {
            queue.push_back((idx % w, idx / w));
        }
    }
    while let Some((cx, cy)) = queue.pop_front() {
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (cx as isize + dx, cy as isize + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                    let ni = ny as usize * w + nx as usize;
                    if candidate[ni] && !kept[ni] {
                        kept[ni] = true;
                        queue.push_back((nx as usize, ny as usize));
                    }
                }
            }
        }
    }
    kept
}

/// Marks transparent pixels reachable from the image boundary (`4`-connected)
/// so interior cutout holes are distinguishable from the outer background.
fn mark_exterior(kept: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut exterior = vec![false; w * h];
    let mut queue = VecDeque::new();
    let mut enqueue = |x: usize, y: usize| {
        let idx = y * w + x;
        if !kept[idx] && !exterior[idx] {
            exterior[idx] = true;
            queue.push_back((x, y));
        }
    };
    for x in 0..w {
        enqueue(x, 0);
        enqueue(x, h - 1);
    }
    for y in 0..h {
        enqueue(0, y);
        enqueue(w - 1, y);
    }
    while let Some((cx, cy)) = queue.pop_front() {
        for (dx, dy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (cx as isize + dx, cy as isize + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                let ni = ny as usize * w + nx as usize;
                if !kept[ni] && !exterior[ni] {
                    exterior[ni] = true;
                    queue.push_back((nx as usize, ny as usize));
                }
            }
        }
    }
    exterior
}

struct SubtractedGrid {
    w: usize,
    h: usize,
    kept: Vec<bool>,
    exterior: Vec<bool>,
    argb: Vec<Option<u32>>,
}

struct GlassParts {
    w: usize,
    h: usize,
    border: Vec<bool>,
    border_rgba: Vec<Option<u32>>,
    palette: Vec<[u8; 3]>,
    ul_streaks: Vec<(usize, usize, [u8; 4])>,
}

fn luma(rgb: [u8; 3]) -> f32 {
    0.2126 * f32::from(rgb[0]) + 0.7152 * f32::from(rgb[1]) + 0.0722 * f32::from(rgb[2])
}

fn lerp_luma_palette(palette: &[[u8; 3]], lum: f32, r_min: f32, r_max: f32) -> [u8; 3] {
    if palette.len() <= 1 {
        return palette[0];
    }
    let t = ((lum - r_min) / (r_max - r_min).max(1.0)).clamp(0.0, 1.0);
    let pos = t * (palette.len() - 1) as f32;
    let idx = (pos as usize).min(palette.len() - 2);
    let frac = pos - idx as f32;
    let (c1, c2) = (palette[idx], palette[idx + 1]);
    [0, 1, 2].map(|i| (f32::from(c1[i]) + (f32::from(c2[i]) - f32::from(c1[i])) * frac).round() as u8)
}

fn encode_runs(grid: &[Option<u32>], w: usize, h: usize) -> Vec<FrameRun> {
    let mut runs = Vec::new();
    for y in 0..h {
        let mut run: Option<(usize, u32)> = None;
        for x in 0..=w {
            let argb = (x < w).then(|| grid[y * w + x]).flatten();
            match run {
                Some((_, c)) if Some(c) == argb => continue,
                Some((start, c)) => {
                    runs.push((start as u16, y as u16, (x - start) as u16, c));
                    run = None;
                }
                None => {}
            }
            if let Some(c) = argb {
                run = Some((x, c));
            }
        }
    }
    runs
}

fn subtract_map_grid(
    map_rgba: &[u8],
    map_size: (u32, u32),
    filled_rgba: &[u8],
    filled_size: (u32, u32),
) -> Option<SubtractedGrid> {
    let (mw, mh) = (map_size.0 as usize, map_size.1 as usize);
    let (fw, fh) = (filled_size.0 as usize, filled_size.1 as usize);
    if mw == 0 || mh == 0 || fw == 0 || fh == 0 || fw > usize::from(u16::MAX) || fh > usize::from(u16::MAX) {
        return None;
    }
    let map_bytes = mw.checked_mul(mh)?.checked_mul(4)?;
    let filled_bytes = fw.checked_mul(fh)?.checked_mul(4)?;
    if map_rgba.len() < map_bytes || filled_rgba.len() < filled_bytes {
        return None;
    }

    let paper_palette: HashSet<[u8; 3]> = map_rgba[..map_bytes]
        .chunks_exact(4)
        .filter(|px| px[3] > 0)
        .map(|px| [px[0], px[1], px[2]])
        .collect();
    if paper_palette.is_empty() {
        return None;
    }

    let total = fw * fh;
    let mut same = vec![false; total];
    let mut candidate = vec![false; total];
    for y in 0..fh {
        let my = (y * mh) / fh;
        for x in 0..fw {
            let mx = (x * mw) / fw;
            let fi = (y * fw + x) * 4;
            let mi = (my * mw + mx) * 4;
            let pf = &filled_rgba[fi..fi + 4];
            if pf[3] == 0 {
                continue;
            }
            let pm = &map_rgba[mi..mi + 4];
            let idx = y * fw + x;
            if pf == pm {
                same[idx] = true;
                candidate[idx] = true;
            } else if paper_palette.contains(&[pf[0], pf[1], pf[2]]) {
                candidate[idx] = true;
            }
        }
    }

    let kept = propagate_kept(&same, &candidate, fw, fh);
    let exterior = mark_exterior(&kept, fw, fh);
    if !(0..total).any(|i| !kept[i] && !exterior[i]) {
        return None;
    }

    let mut argb = vec![None; total];
    for y in 0..fh {
        for x in 0..fw {
            let idx = y * fw + x;
            if !kept[idx] {
                continue;
            }
            let fi = idx * 4;
            let [r, g, b, a] = [
                filled_rgba[fi],
                filled_rgba[fi + 1],
                filled_rgba[fi + 2],
                filled_rgba[fi + 3],
            ];
            let alpha = if fw <= 16 {
                if same[idx] { a } else { (u16::from(a) * 0xA0 / 255) as u8 }
            } else {
                let hole_neighbors = [(-1isize, 0), (1, 0), (0, -1), (0, 1)]
                    .into_iter()
                    .filter(|&(dx, dy)| {
                        let (nx, ny) = (x as isize + dx, y as isize + dy);
                        nx >= 0
                            && ny >= 0
                            && (nx as usize) < fw
                            && (ny as usize) < fh
                            && {
                                let ni = ny as usize * fw + nx as usize;
                                !kept[ni] && !exterior[ni]
                            }
                    })
                    .count();
                if hole_neighbors >= 2 { (u16::from(a) * 0xC8 / 255) as u8 } else { a }
            };
            argb[idx] = Some(u32::from_be_bytes([alpha, r, g, b]));
        }
    }
    Some(SubtractedGrid { w: fw, h: fh, kept, exterior, argb })
}

fn split_glass(glass_rgba: &[u8], glass_size: (u32, u32)) -> Option<GlassParts> {
    let (gw, gh) = (glass_size.0 as usize, glass_size.1 as usize);
    if gw == 0 || gh == 0 || gw > usize::from(u16::MAX) || gh > usize::from(u16::MAX) {
        return None;
    }
    let bytes = gw.checked_mul(gh)?.checked_mul(4)?;
    if glass_rgba.len() < bytes {
        return None;
    }

    let mut border = vec![false; gw * gh];
    let mut queue = VecDeque::new();
    let mut enqueue = |x: usize, y: usize| {
        let idx = y * gw + x;
        if glass_rgba[idx * 4 + 3] > 0 && !border[idx] {
            border[idx] = true;
            queue.push_back((x, y));
        }
    };
    for x in 0..gw {
        enqueue(x, 0);
        enqueue(x, gh - 1);
    }
    for y in 0..gh {
        enqueue(0, y);
        enqueue(gw - 1, y);
    }
    while let Some((cx, cy)) = queue.pop_front() {
        for (dx, dy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (cx as isize + dx, cy as isize + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < gw && (ny as usize) < gh {
                let ni = ny as usize * gw + nx as usize;
                if glass_rgba[ni * 4 + 3] > 0 && !border[ni] {
                    border[ni] = true;
                    queue.push_back((nx as usize, ny as usize));
                }
            }
        }
    }

    let mut colors: Vec<[u8; 3]> = glass_rgba[..bytes]
        .chunks_exact(4)
        .filter(|px| px[3] > 0)
        .map(|px| [px[0], px[1], px[2]])
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if colors.is_empty() {
        return None;
    }
    colors.sort_by(|a, b| luma(*a).total_cmp(&luma(*b)).then_with(|| a.cmp(b)));

    let mut border_rgba = vec![None; gw * gh];
    let mut ul_streaks = Vec::new();
    for y in 0..gh {
        for x in 0..gw {
            let idx = y * gw + x;
            let px = &glass_rgba[idx * 4..idx * 4 + 4];
            if px[3] == 0 {
                continue;
            }
            if border[idx] {
                border_rgba[idx] = Some(u32::from_be_bytes([px[3], px[0], px[1], px[2]]));
            } else if x * gh + y * gw < gw * gh {
                ul_streaks.push((x, y, [px[0], px[1], px[2], px[3]]));
            }
        }
    }

    Some(GlassParts { w: gw, h: gh, border, border_rgba, palette: colors, ul_streaks })
}

/// Derives the tilted map cutout frame at runtime by subtracting `item/map`
/// from `item/filled_map` (each packed RGBA8, top-left origin). Returns
/// horizontal runs of identical ARGB colours.
#[allow(dead_code)]
pub fn subtract_map_frame(
    map_rgba: &[u8],
    map_size: (u32, u32),
    filled_rgba: &[u8],
    filled_size: (u32, u32),
) -> Vec<FrameRun> {
    subtract_map_grid(map_rgba, map_size, filled_rgba, filled_size)
        .map(|g| encode_runs(&g.argb, g.w, g.h))
        .unwrap_or_default()
}

/// Builds the locked full-slot glass overlay from `block/glass`: preserves the
/// edge-connected border and keeps only the upper-left streak smeared right by
/// `width / 16 + 1` pixels.
pub fn locked_full_glass(glass_rgba: &[u8], glass_size: (u32, u32)) -> Vec<FrameRun> {
    let Some(glass) = split_glass(glass_rgba, glass_size) else {
        return Vec::new();
    };
    let mut grid = glass.border_rgba;
    let smear = glass.w / 16 + 1;
    for (sx, sy, [r, g, b, a]) in glass.ul_streaks {
        for dx in 0..smear {
            let nx = sx + dx;
            if nx < glass.w && !glass.border[sy * glass.w + nx] && nx * glass.h + sy * glass.w < glass.w * glass.h {
                grid[sy * glass.w + nx] = Some(u32::from_be_bytes([a, r, g, b]));
            }
        }
    }
    encode_runs(&grid, glass.w, glass.h)
}

fn locked_grid_from(mut base: SubtractedGrid, glass: &GlassParts) -> Vec<FrameRun> {
    let (w, h) = (base.w, base.h);
    let rim_px = (w / 16).max(1);
    let mut dist = vec![usize::MAX; w * h];
    let mut queue = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if !base.kept[idx] {
                continue;
            }
            let touches_ext = x == 0
                || y == 0
                || x == w - 1
                || y == h - 1
                || (-1isize..=1).any(|dy| {
                    (-1isize..=1).any(|dx| {
                        let (nx, ny) = (x as isize + dx, y as isize + dy);
                        nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h
                            && base.exterior[ny as usize * w + nx as usize]
                    })
                });
            if touches_ext {
                dist[idx] = 1;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((cx, cy)) = queue.pop_front() {
        let d = dist[cy * w + cx];
        if d >= rim_px {
            continue;
        }
        for (dx, dy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (cx as isize + dx, cy as isize + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                let ni = ny as usize * w + nx as usize;
                if base.kept[ni] && dist[ni] > d + 1 {
                    dist[ni] = d + 1;
                    queue.push_back((nx as usize, ny as usize));
                }
            }
        }
    }

    let mut r_min = f32::INFINITY;
    let mut r_max = f32::NEG_INFINITY;
    for (idx, &d) in dist.iter().enumerate() {
        if d <= rim_px && let Some(c) = base.argb[idx] {
            let [_, r, g, b] = c.to_be_bytes();
            let lum = luma([r, g, b]);
            r_min = r_min.min(lum);
            r_max = r_max.max(lum);
        }
    }
    if r_min.is_finite() {
        for (idx, &d) in dist.iter().enumerate() {
            if d <= rim_px && let Some(c) = base.argb[idx] {
                let [a, r, g, b] = c.to_be_bytes();
                let [nr, ng, nb] = lerp_luma_palette(&glass.palette, luma([r, g, b]), r_min, r_max);
                base.argb[idx] = Some(u32::from_be_bytes([a, nr, ng, nb]));
            }
        }
    }

    let (sin, cos) = MAP_ITEM_ROTATION_RAD.sin_cos();
    let cx = (w as f32 - 1.0) / 2.0;
    let cy = (h as f32 - 1.0) / 2.0;
    let smear = w / 16 + 1;
    for &(sx, sy, [r, g, b, a]) in &glass.ul_streaks {
        let fx = sx as f32 * w as f32 / glass.w as f32;
        let fy = sy as f32 * h as f32 / glass.h as f32;
        let bx = (cx + (fx - cx) * cos - (fy - cy) * sin).round() as isize;
        let by = (cy + (fx - cx) * sin + (fy - cy) * cos).round() as isize;
        if by < 0 || by as usize >= h {
            continue;
        }
        for dx in 0..smear as isize {
            let nx = bx + dx;
            if nx >= 0 && (nx as usize) < w {
                let ni = by as usize * w + nx as usize;
                if !base.kept[ni] && !base.exterior[ni] {
                    base.argb[ni] = Some(u32::from_be_bytes([a, r, g, b]));
                }
            }
        }
    }

    encode_runs(&base.argb, w, h)
}

/// Derives the locked tilted map frame at runtime by recoloring the subtracted
/// frame's outer rim using `block/glass`'s luminance palette and projecting
/// its rotated, smeared upper-left streak (`width / 16 + 1`) into the cutout hole.
#[allow(dead_code)]
pub fn subtract_locked_map_frame(
    map_rgba: &[u8],
    map_size: (u32, u32),
    filled_rgba: &[u8],
    filled_size: (u32, u32),
    glass_rgba: &[u8],
    glass_size: (u32, u32),
) -> Vec<FrameRun> {
    let Some(base) = subtract_map_grid(map_rgba, map_size, filled_rgba, filled_size) else {
        return Vec::new();
    };
    match split_glass(glass_rgba, glass_size) {
        Some(glass) => locked_grid_from(base, &glass),
        None => encode_runs(&base.argb, base.w, base.h),
    }
}

/// Turns frame pixel runs into filled GUI rectangles at `[at[0], at[1]]` with `step`.
pub fn runs_to_rects(runs: &[FrameRun], at: [f32; 2], step: [f32; 2]) -> Vec<Rect> {
    runs.iter()
        .map(|&(x, y, len, c)| {
            (
                [
                    at[0] + f32::from(x) * step[0],
                    at[1] + f32::from(y) * step[1],
                    f32::from(len) * step[0],
                    step[1],
                ],
                c,
            )
        })
        .collect()
}

/// One texture the slot's frame and glass are cut from: its RGBA pixels, width and height.
pub type Sprite = (Vec<u8>, u32, u32);

/// The textures the slot's map frame and glass are made from, taken from the
/// player's resource packs (`item/map`, `item/filled_map`, `block/glass`).
#[derive(Default)]
pub struct FrameSprites {
    pub map: Option<Sprite>,
    pub filled: Option<Sprite>,
    pub glass: Option<Sprite>,
}

struct CachedFrames {
    item_step: [f32; 2],
    unlocked_item: Vec<FrameRun>,
    locked_item: Vec<FrameRun>,
    full_step: [f32; 2],
    locked_full: Vec<FrameRun>,
}

impl CachedFrames {
    /// The frame (the map item's outline, without its drawing) and the locked
    /// glass, worked out once per set of textures.
    fn build(sprites: &FrameSprites) -> Self {
        let mut built = CachedFrames {
            item_step: [1.0, 1.0],
            unlocked_item: Vec::new(),
            locked_item: Vec::new(),
            full_step: [1.0, 1.0],
            locked_full: Vec::new(),
        };
        let glass = sprites.glass.as_ref().and_then(|(px, w, h)| split_glass(px, (*w, *h)));
        if let (Some(g), Some((px, w, h))) = (&glass, &sprites.glass) {
            built.full_step = [SLOT_SIZE / g.w as f32, SLOT_SIZE / g.h as f32];
            built.locked_full = locked_full_glass(px, (*w, *h));
        }
        if let (Some((m_px, mw, mh)), Some((f_px, fw, fh))) = (&sprites.map, &sprites.filled)
            && let Some(grid) = subtract_map_grid(m_px, (*mw, *mh), f_px, (*fw, *fh))
        {
            built.item_step = [SLOT_SIZE / grid.w as f32, SLOT_SIZE / grid.h as f32];
            built.unlocked_item = encode_runs(&grid.argb, grid.w, grid.h);
            built.locked_item = match &glass {
                Some(g) => locked_grid_from(grid, g),
                None => built.unlocked_item.clone(),
            };
        }
        built
    }
}

/// A map's picture as the client has it.
pub struct MapPicture {
    pub colors: Vec<u8>,
    pub locked: bool,
}

#[derive(Default)]
struct State {
    settings: Option<Settings>,
    /// The map whose tooltip space this mod asked for last: the draw hook
    /// only draws a picture this mod reserved space for.
    claimed: Option<i32>,
    /// The maps' colour palette (`MapColor.col`), from the game.
    palette: Vec<u32>,
    /// The pictures of the maps drawn lately, from the game.
    maps: HashMap<i32, MapPicture>,
    /// The maps asked for since the game patch last ran: it hands over their
    /// pictures (and drops the ones nobody asked for).
    wanted: HashSet<i32>,
    /// The resource packs' revision the frames were made from.
    sprites_revision: Option<u64>,
    frames: Option<CachedFrames>,
    /// The map background's size in its own pixels.
    checker: Option<[f32; 2]>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The kept settings (the defaults before the hooks read them).
pub fn current() -> Settings {
    STATE.with_borrow(|s| s.settings.unwrap_or_default())
}

/// Replaces the settings (read from the page).
pub fn set_settings(settings: Settings) {
    STATE.with_borrow_mut(|s| s.settings = Some(settings));
}

/// The tooltip space this mod asked for map `id`.
pub fn claim(id: i32) {
    STATE.with_borrow_mut(|s| s.claimed = Some(id));
}

/// Whether this mod asked tooltip space for map `id`, so the picture is its to draw.
pub fn claimed(id: i32) -> bool {
    STATE.with_borrow(|s| s.claimed == Some(id))
}

/// Map `id` is about to be drawn: the game patch hands its picture over next frame.
pub fn want(id: i32) {
    STATE.with_borrow_mut(|s| {
        s.wanted.insert(id);
    });
}

/// The maps asked for since last time (the game patch reads them).
pub fn take_wanted() -> Vec<i32> {
    STATE.with_borrow_mut(|s| s.wanted.drain().collect())
}

/// What the game patch hands over: the palette (once), and the pictures of
/// the maps asked for (`maps`); the others are dropped.
pub fn set_maps(palette: &[u32], maps: Vec<(i32, MapPicture)>) {
    STATE.with_borrow_mut(|s| {
        if s.palette.is_empty() {
            s.palette = palette.to_vec();
        }
        s.maps = maps.into_iter().collect();
    });
}

/// Map `id`'s picture and the palette, if the client has it.
pub fn with_map<R>(id: i32, f: impl FnOnce(&MapPicture, &[u32]) -> R) -> Option<R> {
    STATE.with_borrow(|s| s.maps.get(&id).map(|m| f(m, &s.palette)))
}

/// The resource packs' revision the frames were last made from.
pub fn sprites_revision() -> Option<u64> {
    STATE.with_borrow(|s| s.sprites_revision)
}

/// New textures (the resource packs changed): the frames are made again.
pub fn set_sprites(revision: u64, sprites: &FrameSprites, checker: Option<[f32; 2]>) {
    let frames = CachedFrames::build(sprites);
    STATE.with_borrow_mut(|s| {
        s.sprites_revision = Some(revision);
        s.frames = Some(frames);
        s.checker = checker;
    });
}

/// The map background's size in its own pixels.
pub fn checker_size() -> Option<[f32; 2]> {
    STATE.with_borrow(|s| s.checker)
}

/// The slot's frame (as a map item, with the glass on a locked map) and the
/// full-slot glass, at `at`: `(item_rects, locked_full_rects)`. Empty before
/// the textures are in.
pub fn slot_overlays(at: [f32; 2], locked: bool) -> (Vec<Rect>, Vec<Rect>) {
    STATE.with_borrow(|s| {
        let Some(cached) = s.frames.as_ref() else {
            return (Vec::new(), Vec::new());
        };
        let item_runs = if locked { &cached.locked_item } else { &cached.unlocked_item };
        let item_rects = runs_to_rects(item_runs, at, cached.item_step);
        let full_rects = if locked {
            runs_to_rects(&cached.locked_full, at, cached.full_step)
        } else {
            Vec::new()
        };
        (item_rects, full_rects)
    })
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
    fn settings_read_and_page_choices() {
        assert_eq!(Settings::read(stored(&[])), Settings::default());
        let mut s = Settings::read(stored(&[
            ("tooltip", "2"),
            ("tooltip_size", "128\n"),
            ("paper", "1"),
            ("slot", "7"),
            ("slot_style", "2"),
            ("slot_res", "0"),
            ("locked_glass", "0"),
        ]));
        assert_eq!(s.tooltip_side(false), None);
        assert_eq!(s.tooltip_side(true), Some(128.0));
        assert_eq!(s.get(o::SLOT), 1, "not a choice: the default");
        assert_eq!(s.get(o::PAPER), 1);
        assert_eq!(s.get(o::SLOT_STYLE), 2);
        assert_eq!(s.get(o::SLOT_RES), 0);
        assert_eq!(s.get(o::LOCKED_GLASS), 0);
        assert_eq!(Settings::default().tooltip_side(false), Some(96.0));
        // The page keeps choice positions: they read back as these values.
        let page = Settings::from_choices(|key| match key {
            "tooltip" => Some(2),
            "tooltip_size" => Some(2),
            "slot_style" => Some(0),
            "slot" => Some(5),
            _ => None,
        });
        assert_eq!(page.tooltip_side(true), Some(128.0));
        assert_eq!(page.get(o::SLOT_STYLE), 0);
        assert_eq!(page.get(o::SLOT), 1, "not a choice: the default");
        assert_eq!(s.choice_index(o::PAPER), 1);
        s = Settings::default();
        assert_eq!(s.choice_index(o::TOOLTIP_SIZE), 1);
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
        // Left half grass, right half empty (paper shows through on right half).
        let colors = map(|x, _| if x < 64 { 1 << 2 | 2 } else { 0 });

        let t_paper = tooltip_rects(&colors, &PALETTE, [5.0, 5.0], 96.0, 1);
        assert_eq!(t_paper[0], ([5.0, 7.0, 96.0, 96.0], PAPER));
        assert_eq!(t_paper[1], ([5.0, 7.0, 48.0, 1.0], 0xFF7F_B238));

        let t_framed = tooltip_rects(&colors, &PALETTE, [5.0, 5.0], 96.0, 2);
        assert_eq!(tooltip_pad(96.0), 5.0);
        assert_eq!(t_framed[0], ([5.0, 7.0, 106.0, 106.0], PAPER));
        assert_eq!(t_framed[1], ([10.0, 12.0, 48.0, 1.0], 0xFF7F_B238));

        let t_none = tooltip_rects(&colors, &PALETTE, [5.0, 5.0], 96.0, 0);
        assert_eq!(t_none[0], ([5.0, 7.0, 48.0, 1.0], 0xFF7F_B238));
        assert_eq!(tooltip_space(96.0, false), [96.0, 98.0]);
        assert_eq!(tooltip_space(96.0, true), [106.0, 108.0]);

        let s_edge = slot_rects(&colors, &PALETTE, [0.0, 0.0], true, false, false);
        assert_eq!(s_edge[0], ([0.0, 0.0, 16.0, 16.0], PAPER_EDGE));
        assert_eq!(s_edge[1], ([1.0, 1.0, 14.0, 14.0], PAPER));
        assert_eq!(s_edge[2], ([1.0, 1.0, 7.0, 1.0], 0xFF7F_B238));

        let full = slot_rects(&colors, &PALETTE, [0.0, 0.0], false, false, false);
        assert_eq!(full[0], ([0.0, 0.0, 8.0, 1.0], 0xFF7F_B238));

        let high = slot_rects(&colors, &PALETTE, [0.0, 0.0], false, false, true);
        assert_eq!(high.len(), SLOT_HIGH_CELLS, "one run per row of a two-colour map");
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

    fn rgba_grid(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 4]) -> Vec<u8> {
        (0..w * h).flat_map(|i| f(i % w, i / w)).collect()
    }

    #[test]
    fn subtract_map_frame_cuts_terrain_and_keeps_bevel() {
        let light = [242, 226, 194, 255];
        let bevel = [212, 192, 157, 255];
        let terrain = [30, 138, 22, 255];
        let clear = [0, 0, 0, 0];

        // 16x16: outer ring matches `map` (which contains both `light` and `bevel`
        // shades), (2,2) is bevel, (2..14,2..14) is terrain except (8,8) where `map`
        // had `bevel` and `filled` painted `light` inside terrain (must be erased).
        let map_16 = rgba_grid(16, 16, |x, y| {
            if x == 0 && y == 0 {
                clear
            } else if (x == 1 && y == 1) || (x == 8 && y == 8) {
                bevel
            } else {
                light
            }
        });
        let filled_16 = rgba_grid(16, 16, |x, y| {
            if x == 0 && y == 0 {
                clear
            } else if x == 1 && y == 1 {
                bevel
            } else if x >= 2 && x < 14 && y >= 2 && y < 14 {
                if x == 2 && y == 2 { bevel } else if x == 8 && y == 8 { light } else { terrain }
            } else {
                light
            }
        });

        let runs_16 = subtract_map_frame(&map_16, (16, 16), &filled_16, (16, 16));
        assert!(!runs_16.is_empty());
        // (2,2) bevel pixel gets 0xA0 alpha; (8,8) isolated interior parchment pixel is erased.
        let mut pixels = vec![0u32; 256];
        for (x, y, len, argb) in &runs_16 {
            for dx in 0..*len {
                pixels[*y as usize * 16 + *x as usize + dx as usize] = *argb;
            }
        }
        assert_eq!(pixels[0], 0);
        assert_eq!(pixels[1], 0xFFF2_E2C2);
        assert_eq!(pixels[2 * 16 + 2], 0xA0D4_C09D);
        assert_eq!(pixels[8 * 16 + 8], 0);

        // 32x32: staircase hole corner gets 0xC8 alpha; flat interior edge stays 0xFF.
        let map_32 = rgba_grid(32, 32, |_, _| light);
        let filled_32 = rgba_grid(32, 32, |x, y| {
            let in_hole = (4..28).contains(&x) && (4..28).contains(&y) && !(x == 4 && y == 4);
            if in_hole { terrain } else { light }
        });
        let runs_32 = subtract_map_frame(&map_32, (32, 32), &filled_32, (32, 32));
        let mut px32 = vec![0u32; 32 * 32];
        for (x, y, len, argb) in &runs_32 {
            for dx in 0..*len {
                px32[*y as usize * 32 + *x as usize + dx as usize] = *argb;
            }
        }
        // (4,4) has two 4-connected hole neighbors ((5,4) and (4,5)) -> 0xC8.
        assert_eq!(px32[4 * 32 + 4], 0xC8F2_E2C2);
        // (6,3) has only one 4-connected hole neighbor ((6,4)) -> 0xFF.
        assert_eq!(px32[3 * 32 + 6], 0xFFF2_E2C2);

        // Mismatched sizes (16x16 map vs 32x32 filled_map) and caching work cleanly.
        let runs_mixed = subtract_map_frame(&map_16, (16, 16), &filled_32, (32, 32));
        assert!(!runs_mixed.is_empty());
        assert!(subtract_map_frame(&map_16, (16, 16), &map_16, (16, 16)).is_empty());

        let glass_bright = [208, 234, 233, 255];
        let glass_dark = [123, 174, 183, 255];
        let glass_16 = rgba_grid(16, 16, |x, y| {
            if x == 0 || y == 0 {
                glass_bright
            } else if x == 15 || y == 15 {
                glass_dark
            } else if (x, y) == (4, 2) || (x, y) == (13, 12) {
                glass_bright
            } else {
                clear
            }
        });
        let full_glass = locked_full_glass(&glass_16, (16, 16));
        let mut gpx = vec![0u32; 256];
        for (x, y, len, argb) in &full_glass {
            for dx in 0..*len {
                gpx[*y as usize * 16 + *x as usize + dx as usize] = *argb;
            }
        }
        // Border kept, UL streak (4,2) smeared by 16/16 + 1 = 2px -> (4,2) and (5,2), LR streak (13,12) removed.
        assert_eq!(gpx[0], 0xFFD0_EAE9);
        assert_eq!(gpx[2 * 16 + 4], 0xFFD0_EAE9);
        assert_eq!(gpx[2 * 16 + 5], 0xFFD0_EAE9);
        assert_eq!(gpx[12 * 16 + 13], 0);

        let locked_16 = subtract_locked_map_frame(&map_16, (16, 16), &filled_16, (16, 16), &glass_16, (16, 16));
        let mut lpx = vec![0u32; 256];
        for (x, y, len, argb) in &locked_16 {
            for dx in 0..*len {
                lpx[*y as usize * 16 + *x as usize + dx as usize] = *argb;
            }
        }
        // Outer rim pixel (1,0) recolored into glass palette; interior bevel (2,2) stays parchment.
        assert_ne!(lpx[1], 0xFFF2_E2C2);
        assert_eq!(lpx[2 * 16 + 2], 0xA0D4_C09D);

        assert_eq!(slot_overlays([10.0, 20.0], true), (Vec::new(), Vec::new()), "no textures yet");
        let sprites = FrameSprites {
            map: Some((map_16.clone(), 16, 16)),
            filled: Some((filled_16.clone(), 16, 16)),
            glass: Some((glass_16.clone(), 16, 16)),
        };
        set_sprites(99, &sprites, Some([64.0, 64.0]));
        assert_eq!(sprites_revision(), Some(99));
        assert_eq!(checker_size(), Some([64.0, 64.0]));
        let (item_rects, full_rects) = slot_overlays([10.0, 20.0], true);
        assert!(!item_rects.is_empty());
        assert!(!full_rects.is_empty());
        let (unlocked, no_glass) = slot_overlays([10.0, 20.0], false);
        assert!(!unlocked.is_empty());
        assert!(no_glass.is_empty());
    }

    #[test]
    fn claims_and_the_maps_handed_over() {
        claim(7);
        assert!(claimed(7));
        assert!(!claimed(8));
        // Maps asked for are handed over by the game patch; the others are dropped.
        want(3);
        want(3);
        want(5);
        let mut wanted = take_wanted();
        wanted.sort();
        assert_eq!(wanted, [3, 5]);
        assert!(take_wanted().is_empty());
        set_maps(&[1, 2], vec![(3, MapPicture { colors: vec![4; 9], locked: true })]);
        assert_eq!(with_map(3, |m, palette| (m.colors.len(), m.locked, palette.len())), Some((9, true, 2)));
        assert!(with_map(5, |_, _| ()).is_none());
        set_maps(&[9], Vec::new());
        assert!(with_map(3, |_, _| ()).is_none(), "not asked for again: dropped");
        assert_eq!(with_map(3, |_, p| p.len()), None);
    }

    #[test]
    fn every_label_is_in_english() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/map_preview/lang/en_us.json");
        let file = std::fs::read_to_string(path).unwrap_or_default();
        for (key, text) in english() {
            let line = format!("{key:?}: {text:?}");
            assert!(file.contains(&line), "{line} missing from en_us.json");
        }
    }
}
