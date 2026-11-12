// A map's slot (hotbar, inventory, containers): its picture over the plain
// map icon, when the client has the map.
fn map_preview_slot(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    x: f32,
    y: f32,
    batch: &mut horizon_client::hud::HudBatch,
) {
    use crate::game::map_preview as m;
    if !game.native_on(m::ID) {
        return;
    }
    let Some(id) = crate::game::maps::map_id(stack) else {
        return;
    };
    let settings = m::settings(|key| game.native_setting(m::ID, key));
    if settings.get(m::o::SLOT) == 0 {
        return;
    }
    let Some(map) = game.maps.data.get(&id) else {
        return;
    };

    // The slot's item count text was painted immediately before this hook.
    // Pop trailing text vertices within the slot so the map preview draws behind them.
    let mut split_at = batch.vertices.len();
    while split_at > 0 {
        let v = &batch.vertices[split_at - 1];
        if v.text == 1
            && v.pos[0] >= x - 2.0
            && v.pos[0] <= x + 24.0
            && v.pos[1] >= y - 2.0
            && v.pos[1] <= y + 24.0
        {
            split_at -= 1;
        } else {
            break;
        }
    }
    let decorations = batch.vertices.split_off(split_at);

    let style = settings.get(m::o::SLOT_STYLE);
    let high_res = settings.get(m::o::SLOT_RES) == 1;
    let locked_glass = settings.get(m::o::LOCKED_GLASS) == 1;
    let is_locked = map.locked || stack.data().and_then(|d| d.map_post_processing) == Some(0);
    let palette = &horizon_client::blocks::MAP_COLORS;

    if style == 2 {
        let (paper_quad, paper_color) = m::tilted_paper_quad([x, y]);
        game.native_quad(batch, paper_quad, paper_color);

        let cells = if high_res { m::MAP_SIZE } else { 13 };
        for (quad, argb) in m::tilted_picture(&map.colors, palette, [x, y], cells) {
            game.native_quad(batch, quad, argb);
        }

        let res = game.gui.atlas.sprites.get("item/filled_map")
            .or_else(|| game.gui.atlas.sprites.get("item/map"))
            .map_or(16, |r| r.texels((game.gui.atlas.width, game.gui.atlas.height)).0);
        let (frame, step) = m::frame_runs(res);
        for (rect, argb) in m::runs_to_rects(frame, [x, y], step) {
            game.native_fill(batch, rect, argb);
        }

        if is_locked && locked_glass {
            for (rect, argb) in m::runs_to_rects(m::LOCKED_GLASS, [x, y], 1.0) {
                game.native_fill(batch, rect, argb);
            }
        }
    } else {
        let edged = style == 1;
        let paper = settings.get(m::o::PAPER) != 0;
        for (rect, argb) in m::slot_rects(&map.colors, palette, [x, y], edged, paper, high_res) {
            game.native_fill(batch, rect, argb);
        }

        if is_locked && locked_glass {
            for (rect, argb) in m::runs_to_rects(m::LOCKED_GLASS_FULL, [x, y], 1.0) {
                game.native_fill(batch, rect, argb);
            }
        }
    }

    batch.vertices.extend(decorations);
}
