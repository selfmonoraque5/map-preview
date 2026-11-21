//! Map Preview on Horizon's mod API: a Part on the filled map item draws its
//! picture in the tooltip and in the slot. The client's map pictures come from
//! the game patch in `src/mixins/maps.rs` (see horizon-mod.toml), as the mod
//! API has no client read of a map's pixels.
use mod_api::{
    self, ClientHost, ClientTickPostEvent, Hooks, ItemDecorationsEvent, ItemPart, ItemStack,
    Registrar, RegistrationContext, SettingChangedEvent, SettingSpec, Tag, Text,
    TooltipComponentDrawEvent, TooltipComponentEvent,
};

/// The mod's logic, also read by the game patch.
pub mod preview;

use preview::{self as m, OPTS, Settings};

/// The settings as the page keeps them.
fn read(host: &dyn ClientHost) -> Settings {
    Settings::from_choices(|key| host.setting_choice(key))
}

/// An integer component of `stack` (`minecraft:map_id`).
fn int_component(stack: &ItemStack, id: &str) -> Option<i32> {
    let patch = stack.components()?.as_nbt();
    patch.iter().find_map(|(key, value)| match value {
        Tag::Int(v) if key.to_string_lossy() == id => Some(*v),
        _ => None,
    })
}

/// The map a stack shows.
fn map_id(stack: &ItemStack) -> Option<i32> {
    int_component(stack, "minecraft:map_id")
}

/// A map locked in a cartography table, the client may not know it yet: the
/// stack still carries the lock it is getting (`map_post_processing` 0).
fn locking(stack: &ItemStack) -> bool {
    int_component(stack, "minecraft:map_post_processing") == Some(0)
}

#[derive(Default)]
struct MapPreview;

#[mod_api::horizon_mod]
impl Hooks for MapPreview {
    /// The settings page (one choice per option) and the map item's Part.
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let defaults = Settings::default();
        for (i, opt) in OPTS.iter().enumerate() {
            let options = opt.choices.iter().map(|(v, _)| m::choice_key(opt.key, *v));
            let _ = r.register_setting(SettingSpec::choice(
                opt.key,
                m::label_key(opt.key),
                options,
                defaults.choice_index(i),
            ));
        }
        let map = r.item_key("minecraft:filled_map");
        r.item_part(map, FilledMap);
    }

    /// The player changed an option on the page.
    fn setting_changed(&self, host: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        m::set_settings(read(&*host));
    }

    /// Each tick, the title screen too: the settings.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        m::set_settings(read(&*host));
    }
}

/// The filled map item: its tooltip picture and its slot picture.
struct FilledMap;

#[mod_api::horizon_part]
impl ItemPart for FilledMap {
    /// A map's tooltip asks for space below its lines for the picture, when the
    /// client has the map and the settings show it.
    fn tooltip_component(&self, host: &dyn ClientHost, event: &mut TooltipComponentEvent<'_>) -> Option<[f32; 2]> {
        let id = map_id(event.stack)?;
        m::want(id);
        m::with_map(id, |_, _| ())?;
        let settings = m::current();
        let side = settings.tooltip_side(host.modifiers().shift())?;
        let space = m::tooltip_space(side, settings.get(m::o::PAPER) == 2);
        m::claim(id, space);
        Some(space)
    }

    /// The tooltip's picture, in the space `tooltip_component` asked for.
    fn tooltip_component_draw(&self, _: &dyn ClientHost, event: &mut TooltipComponentDrawEvent<'_>) {
        let Some(id) = map_id(event.stack) else {
            return;
        };
        // Only the space this mod asked for: another mod's tooltip part for
        // the same map has its own.
        if !m::claimed(id, event.size) {
            return;
        }
        m::want(id);
        let settings = m::current();
        let Some(side) = settings.tooltip_side(true) else {
            return;
        };
        let bg = settings.get(m::o::PAPER);
        let draw = &mut *event.draw;
        let top = [event.origin[0], event.origin[1] + m::TOOLTIP_GAP];
        m::with_map(id, |map, palette| {
            let (map_at, map_side) = if bg == 2 {
                let pad = m::tooltip_pad(side);
                let total = side + 2.0 * pad;
                let [sw, sh] = m::checker_size().unwrap_or([64.0, 64.0]);
                let back = [top[0], top[1], total, total];
                if !draw.sprite("map/map_background_checkerboard", [0.0, 0.0, sw, sh], back, None) {
                    draw.fill(back, m::PAPER);
                }
                ([top[0] + pad, top[1] + pad], side)
            } else {
                if bg == 1 {
                    draw.fill([top[0], top[1], side, side], m::PAPER);
                }
                (top, side)
            };
            let cells = (map_side.round() as usize).min(m::MAP_SIZE);
            for (rect, argb) in m::picture(&map.colors, palette, map_at, map_side, cells) {
                draw.fill(rect, argb);
            }
        });
    }

    /// A map's slot (hotbar, inventory, containers): its picture over the
    /// plain map icon, when the client has the map, then the stack's count
    /// again on top (the game drew it before this).
    fn item_decorations(&self, host: &dyn ClientHost, event: &mut ItemDecorationsEvent<'_>) {
        let settings = m::current();
        if settings.get(m::o::SLOT) == 0 {
            return;
        }
        let Some(id) = map_id(event.stack) else {
            return;
        };
        m::want(id);
        let (x, y) = (event.x, event.y);
        let draw = &mut *event.draw;
        let style = settings.get(m::o::SLOT_STYLE);
        let high_res = settings.get(m::o::SLOT_RES) == 1;
        let glass = settings.get(m::o::LOCKED_GLASS) == 1;
        let drawn = m::with_map(id, |map, palette| {
            let locked = glass && (map.locked || locking(event.stack));
            let (item_rects, full_rects) = if style == 2 || locked {
                m::slot_overlays([x, y], locked)
            } else {
                (Vec::new(), Vec::new())
            };
            if style == 2 && !item_rects.is_empty() {
                let (paper, paper_color) = m::tilted_paper_quad([x, y]);
                draw.quad(paper, paper_color);
                let cells = if high_res { m::SLOT_HIGH_CELLS } else { 13 };
                for (quad, argb) in m::tilted_picture(&map.colors, palette, [x, y], cells) {
                    draw.quad(quad, argb);
                }
                for (rect, argb) in item_rects {
                    draw.fill(rect, argb);
                }
            } else {
                // "As map item" falls back to the full slot until the map's textures are in.
                let edged = style != 0;
                let paper = settings.get(m::o::PAPER) != 0;
                for (rect, argb) in m::slot_rects(&map.colors, palette, [x, y], edged, paper, high_res) {
                    draw.fill(rect, argb);
                }
                for (rect, argb) in full_rects {
                    draw.fill(rect, argb);
                }
            }
        });
        // The count, as vanilla draws it: right-aligned at the slot's bottom right, with its shadow.
        let count = event.stack.count();
        if drawn.is_some() && count > 1 {
            let text = count.to_string();
            let width = host.text_width(&Text::literal(text.as_str())).unwrap_or(6) as f32;
            draw.text(&text, [x + 19.0 - 2.0 - width, y + 6.0 + 3.0], 0xFF_FF_FF_FF, true);
        }
    }
}
