# Publishing Map Preview to the Horizon registry

Same steps as Living Camera.

## 1. Source on GitHub

Create a public repository `map-preview` (owner selfmonoraque5; README, .gitignore and license all off), then commit
everything in this folder except the `.hnmod` (GitHub Desktop or "uploading an existing file"). Check that
`mixins/map_preview/` shows 11 files. Optionally make a release `v1.0.0` with the `.hnmod` attached.

## 2. Submit

https://playhorizon.dev/mods/submit with `map_preview-1.0.0.hnmod`.

**What changed:** First release.

**Name:** Map Preview

**Licence:** MIT

**Summary:**
See a map's picture in its tooltip and in its inventory slot, so you can tell your maps apart.

**Description:**

```
See what's on a map without holding it.

- **Tooltip preview**: hover a filled map and its picture shows in the tooltip. Always, or only while holding Shift, in three sizes.
- **Slot preview**: every map in your hotbar, inventory and chests shows a small copy of its picture instead of the plain map icon, with or without a paper edge.
- Maps update live as you explore.

All in Options > Mods > Map Preview, with a reset button.

Client side only: it works in single player and on any server. A preview needs the map's picture on your client: the server sends it for maps you carry, as vanilla does, so a map you've never carried may show no preview until you pick it up once.

Horizon has no hook point for a map's pixels, so besides hook points this mod reads the game's own map data (Game.maps, maps::map_id) and palette (MAP_COLORS); a later Horizon that changes those needs an update of this mod.

Source: https://github.com/selfmonoraque5/map-preview

Inspired by Map Tooltip by VendoAU (https://modrinth.com/mod/map-tooltip) and Map In Slot by Crec0 (https://modrinth.com/mod/map-in-slot). Written from scratch for Horizon: it contains none of their code or assets and is not made or endorsed by them.
```

**Categories:** Utility

**Tags:** maps, tooltip, inventory, preview, client-side

**Authors:** selfmonoraque5

**Source code:** `https://github.com/selfmonoraque5/map-preview`

**Icon:** `icon.png`

Leave the signature empty, as for Living Camera.
