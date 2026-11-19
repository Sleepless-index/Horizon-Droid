# Polytone port: status and next steps

Polytone is Java code, so it cannot be a `.hmod`. Horizon reads the Polytone pack format natively in
`crates/horizon-polytone`, written clean-room from the public wiki
(https://github.com/MehVahdJukaar/polytone/wiki) and from packs. Do not read or decompile the mod.
Work only in this repository.

## Done

- Expressions, targets (id, `#tag`, regex), priority resolution, attribute layers.
- Colormaps (`polytone/colormaps`), `colors.json` biome colours, fluid modifiers.
- Applied in game through `crates/horizon-client-render/src/biome.rs`:
  - grass, foliage, dry foliage and water colour from `colors.json` and biome-only colormaps
    (a biome's own colour still wins, water is replaced everywhere);
  - water tint from a fluid modifier whose colormap depends only on the biome;
  - sky, fog and water fog colour from `biome_modifiers` (`biome_attributes.rs`): override layers and the colour
    modifiers `add`, `subtract`, `multiply` (a colour or a number), `minimum`, `maximum` and `alpha_blend`, applied in
    order (`base` then `post_process`) to the colour the biome has, highest priority file wins. A sky or fog the biome
    leaves to its dimension has nothing to edit, so only an override sets it. Targets by id, `#tag` (from `tags/worldgen/biome`) and regex.
- Block modifier `colormap` tints (`MeshTables::apply_polytone_block_tints`): a flat colour or a
  colormap per tint index 0 and 1, blended like vanilla biome tints; a colormap whose axes read the position
  (`POS_X`, `POS_Y`, `POS_Z`, `o.getX()`...) is sampled at each blended block's world position. Targets by id, `#tag`
  and regex.
- Fluid modifiers: lava and flowing lava take a `colormap` tint of their own (sampled with plains climate), and the
  `fog_colormap` (flat) and `fog_radius` of water and lava recolour and scale the fog under the fluid.
- Item modifier `colormap` tints (`item_tints.rs`, applied in `ItemRenderer::resolve`): tint indices 0 to 4 replace the
  item model's tints, sampled at the owner's position with plains climate. Targets by id, `#tag` and regex.
- Item modifier `bar_color` (a flat colour only; the `damage` type and textures leave vanilla's bar) and `rarity`
  (name colour of the held-item name, tooltip and chat links).
- Block modifier `render_type` (solid, cutout, translucent: which terrain layer the block's faces draw in) and
  `offset_type` (`none`, `xz`, `xyz`; the model offset only, collision and the outline keep vanilla's).
- Dimension modifiers (`dimension_attributes.rs`, applied in `Environment::load_dimension` and `sample`): `base` edits the
  dimension's own fog start, fog end, cloud fog end and sky, fog and cloud colours; `post_process` edits the final
  sampled values. The `rain` and `thunder` groups and the other attributes are not applied.
- Creative tab modifiers (`Creative::load`): `removals`, `additions` (before or after a predicate match, new stacks as
  plain items) and `icon` change a category tab's stacks and icon; the search, inventory and hotbar tabs are left
  alone. A tab's `name` isn't applied.
- Screen sprite modifiers (`overlay_modifiers`, applied once when the atlas builds, `HudAtlas::apply_sprite_modifiers`):
  `color` (a multiply on the sprite's texels), `new_texture` (the sprite draws another's region), `u0` to `v1` and
  `width_inc`, `height_inc`. Not applied: `x_inc`, `y_inc`, `overlays`, and `color` together with `new_texture`.
- `colors.json` / `color.properties` custom colours: `environment.*` (`powder_snow_fog`, `blindness_fog_distance`,
  `darkness_fog_distance`, `boss_fog_distance`, `water_fog_brightening`) in `FogTables`, and `redstone.<power>` for the
  wire's tint, `effect.*` (mob effect colours in the effect table), `dye.*` diffuse colours (mobs, shulker/banner
  blocks, dyed leather crafting), `dye.*.text` (sign text), `dye.*.firework` (stars, tooltips, raids) and `sheep.*`
  wool colours, `particle.*` (a colour or expression, applied as a `color` modifier) and the redstone dust particle colour.
- Item modifier tooltips: `tooltips` add lines (text or component, colour, bold, italic; `position` counts lines under the
  name, negative from the end) and `removed_tooltips` patterns drop lines whose text a matching language key shows or
  that match the pattern themselves.
- Pack config: Horizon > Pack Config... lists the selected packs' entries (toggle, step or cycle; Shift-click resets one,
  Reset all, and a Preset button when entries have presets); leaving the page with changes saves and reloads the packs.
  `config_entries/` are read at their defaults or the player's choices (`polytone-configs.json` beside
  `options.txt`), `polytone_condition` filters any scanned file, particle modifiers can call `config(id)`, `overlays.entries[].polytone_condition` picks the pack's overlay
  folders and `sounds.json` events with an unmet `polytone_condition` are dropped before the packs merge (the overlay
  cache keys on the player's choices).
- Particle modifiers (`particle_modifiers/`): `color`, `red/green/blue/alpha`, `size`, `life` and `speed` expressions run
  once when a particle is created, with `o`, `p`, `g` and `r` answered (no biome/light/colormap/config yet).
  `horizon_height` (the dark disc's height) and `water_fog_brightening_time` (ticks to full water vision) apply too.
- Read but not applied: the other block fields (`map_color`, `can_occlude`, particles), item modifier
  arm pose and armor/scope textures, flowing-water-only tints, `fog_fade`, `sprites` offsets and overlays,
  per-tick particle expressions, `config(...)` in expressions other than particle modifiers
  and conditions.

## Next, in rough order

1. Custom `offset_type` objects; `can_occlude`.
2. Textured `bar_color`, arm pose, armor/scope textures.
3. Position-dependent colormaps for fluid tints (blocks and items already read the position).
4. Refused colormap features: `color_modifier`, `biome_id`, `legacy_temperature`, `grid`, `damage`.
5. Biome layers: `blend_to_gray`, `REMOVE`, colormap and expression arguments, other attributes; weather groups and the other dimension attributes; fog distances and cloud colour.
6. Per-tick particle expressions.

## Rules

- Tests: `cargo +stable test -p horizon-polytone` and
  `cargo +stable test -p horizon-client-render --lib biome` (the pinned 1.95.0 toolchain may not
  download here; stable's clippy flags unrelated `incompatible_msrv` and `unexpected_cfgs`).
- Pull before starting, push to `android` when done; small commits.
