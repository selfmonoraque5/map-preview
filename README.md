<p align="center"><img src="icon.png" width="128" alt="Map Preview icon"></p>

# Map Preview (Horizon native mod)

See what's on a map without holding it. Hover a filled map and its picture shows in the tooltip; every map in your
hotbar, inventory and chests shows a small copy of its picture instead of the plain map icon, so you can tell them
apart at a glance.

Client side only: it works in single player and on any server.

## Settings

Options > Mods > Map Preview > Settings. Each button steps through its values.

| Setting | Values | Default |
|---|---|---|
| Tooltip preview | OFF, Always, Hold Shift | Always |
| Tooltip size | Small (64), Medium (96), Large (128 GUI pixels) | Medium |
| Slot preview | OFF, ON | ON |
| Slot style | Full slot, With paper edge | With paper edge |
| Paper behind the map | OFF, ON | ON |
| Reset to defaults | | |

## Good to know

- A preview needs the map's picture on your client. The server sends it for maps you carry (as vanilla does), so
  your own maps always show. A map you have never carried, in a chest on a server, may show no preview until you
  pick it up once.
- Maps update live: explore with a map and its preview fills in.

## Install

In the Horizon launcher: Mods, find **Map Preview**, Install, then **Build**. To try a local copy, put the `.hnmod` in
your instance's `horizon-native-mods/` folder and Build.

## Compatibility

Tested on Horizon Nightly `fdc87ae` (2026-11-08), hook API 411, horizon-mixins `5a64af9`.

Horizon has no hook point that gives a mod a map's pixels, so besides hook points this mod reads the game's own maps
(`Game.maps` and `maps::map_id` in `crates/horizon-client/src/game/maps.rs`) and Horizon's map palette
(`horizon_client::blocks::MAP_COLORS`). If a later Horizon changes those, this mod needs an update for it.

## How it works

| File | What it is |
|---|---|
| `mixins/map_preview/horizon-mixin.toml` | The bundle: one module and its HEAD hooks |
| `mixins/map_preview/runtime.rs` | The settings, the map colours and the picture as rectangles; also this crate's library, for its tests |
| `measure.rs`, `draw.rs` | The tooltip picture (`native_tooltip_component` and `native_tooltip_component_draw`) |
| `slot.rs` | The slot picture (`native_item_decorations`) |
| `key.rs` | Follows Shift, for "Hold Shift" |
| `list.rs`, `settings.rs`, `click.rs` | The Options > Mods page |

Pictures are drawn as filled rectangles, one per run of same-coloured pixels in a row. The mod only reads map data the
client already has; it has no network, file or process access beyond Horizon's own settings files for the mod.

```sh
cargo test    # colours, picture layout and settings
```

## Credits

Inspired by [Map Tooltip](https://modrinth.com/mod/map-tooltip) by VendoAU and
[Map In Slot](https://modrinth.com/mod/map-in-slot) by Crec0. Map Preview is written from scratch for Horizon and
contains none of their code or assets; it is not made or endorsed by them.

## License

MIT, see [LICENSE](LICENSE).
