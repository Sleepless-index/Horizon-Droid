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
  - sky, fog and water fog colour from `biome_modifiers` (`biome_attributes.rs`): plain colour or
    number override layers, `base` then `post_process`, highest priority wins. Targets by id, `#tag` (from `tags/worldgen/biome`) and regex.
- Block modifier `colormap` tints (`MeshTables::apply_polytone_block_tints`): a flat colour or a
  biome-only colormap per tint index 0 and 1, blended like vanilla biome tints. Targets by id, `#tag`
  and regex.
- Item modifier `colormap` tints (`item_tints.rs`, applied in `ItemRenderer::resolve`): tint indices 0 to 4 replace the
  item model's tints, sampled at the owner's position with plains climate. Targets by id, `#tag` and regex.
- Item modifier `bar_color` (a flat colour only; the `damage` type and textures leave vanilla's bar) and `rarity`
  (name colour of the held-item name, tooltip and chat links).
- Block modifier `render_type` (solid, cutout, translucent: which terrain layer the block's faces draw in) and
  `offset_type` (`none`, `xz`, `xyz`; the model offset only, collision and the outline keep vanilla's).
- Read but not applied: the other block fields (`map_color`, `can_occlude`, particles), item modifier
  tooltips, arm pose and armor/scope textures, fluid modifiers other than the water tint, `sprites`, `creative_tabs`,
  `particles`, `pack` config, dimension modifiers, `colors.json` custom colours.

## Next, in rough order

1. Block tints that read the position; custom `offset_type` objects; `can_occlude`.
2. Item modifier tooltips, textured `bar_color`, arm pose, armor/scope textures.
3. Position-dependent colormaps (`POS_X`, `POS_Z`, ...), now logged as "isn't applied yet".
4. Refused colormap features: `color_modifier`, `biome_id`, `legacy_temperature`, `grid`, `damage`.
5. Biome layers that modify (`add`, `multiply`, `alpha_blend`...), `REMOVE`, colormap and expression
   arguments; dimension modifiers and weather groups; fog distances and cloud colour.
6. Wire `sprites`, `creative_tabs`, `particles`, `colors.json` custom colours.

## Rules

- Tests: `cargo +stable test -p horizon-polytone` and
  `cargo +stable test -p horizon-client-render --lib biome` (the pinned 1.95.0 toolchain may not
  download here; stable's clippy flags unrelated `incompatible_msrv` and `unexpected_cfgs`).
- Pull before starting, push to `android` when done; small commits.
