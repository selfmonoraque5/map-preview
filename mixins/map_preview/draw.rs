// The tooltip's picture, in the space `measure.rs` asked for (not in another
// mod's).
fn map_preview_draw(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
    size: [f32; 2],
) {
    use crate::game::map_preview as m;
    if !game.native_on(m::ID) {
        return;
    }
    let Some(id) = crate::game::maps::map_id(stack) else {
        return;
    };
    if !m::claimed(id, size) {
        return;
    }
    let Some(map) = game.maps.data.get(&id) else {
        return;
    };
    let settings = m::settings(|key| game.native_setting(m::ID, key));
    let bg = settings.get(m::o::PAPER);
    let top = [origin[0], origin[1] + m::TOOLTIP_GAP];
    let side = settings.tooltip_side(true).unwrap_or(size[0]);

    let (map_at, map_side) = if bg == 2 {
        let pad = m::tooltip_pad(side);
        let total = side + 2.0 * pad;
        let [sw, sh] = game.native_sprite_size("map/map_background_checkerboard")
            .unwrap_or([64.0, 64.0]);
        if !game.native_sprite(
            batch,
            "map/map_background_checkerboard",
            [0.0, 0.0, sw, sh],
            [top[0], top[1], total, total],
        ) {
            game.native_fill(batch, [top[0], top[1], total, total], m::PAPER);
        }
        ([top[0] + pad, top[1] + pad], side)
    } else {
        if bg == 1 {
            game.native_fill(batch, [top[0], top[1], side, side], m::PAPER);
        }
        (top, side)
    };

    let palette = &horizon_client::blocks::MAP_COLORS;
    let cells = (map_side.round() as usize).min(m::MAP_SIZE);
    for (rect, argb) in m::picture(&map.colors, palette, map_at, map_side, cells) {
        game.native_fill(batch, rect, argb);
    }
}
