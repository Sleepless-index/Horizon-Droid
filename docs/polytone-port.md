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
  `width_inc`, `height_inc`. Not applied: `color` together with `new_texture`.
- `colors.json` / `color.properties` custom colours: `environment.*` (`powder_snow_fog`, `blindness_fog_distance`,
  `darkness_fog_distance`, `boss_fog_distance`, `water_fog_brightening`) in `FogTables`, and `redstone.<power>` for the
  wire's tint, `effect.*` (mob effect colours in the effect table), `dye.*` diffuse colours (mobs, shulker/banner
  blocks, dyed leather crafting), `dye.*.text` (sign text), `dye.*.firework` (stars, tooltips, raids) and `sheep.*`
  wool colours, `particle.*` (a colour or expression, applied as a `color` modifier) and the redstone dust particle colour.
- Item modifier tooltips: `tooltips` add lines (text or component, colour, bold, italic; `position` counts lines under the
  name, negative from the end) and `removed_tooltips` patterns drop lines whose text a matching language key shows or
  that match the pattern themselves.
- Item `arm_pose`: a held item's pose replaces vanilla's choice for player models in third person (`spear` only for
  items that have kinetic data, `spyglass` shows as a plain held item); first-person arms are unchanged.
- Item `bob_as_entity: false` keeps a dropped item at rest height (it still spins); `spread_as_entity: false` draws a
  dropped stack as one copy.
- Item `armor_texture`: `ns:name` replaces the first layer of the worn armour with `ns:entity/equipment/<humanoid|humanoid_leggings>/name`
  (a path starting `entity/` is used as is) on players, armour stands and vanilla-armoured mobs; `scope_overlay`: `ns:path` is the
  texture a spyglass shows when scoped. Any pack namespace's `entity/equipment` and `misc` textures now reach the atlas.
- Pack config: Horizon > Pack Config... lists the selected packs' entries (toggle, step or cycle; Shift-click resets one,
  Reset all, and a Preset button when entries have presets); leaving the page with changes saves and reloads the packs.
  `config_entries/` are read at their defaults or the player's choices (`polytone-configs.json` beside
  `options.txt`), `polytone_condition` filters any scanned file, particle modifiers can call `config(id)`, `overlays.entries[].polytone_condition` picks the pack's overlay
  folders and `sounds.json` events with an unmet `polytone_condition` are dropped before the packs merge (the overlay
  cache keys on the player's choices).
- Textured `bar_color`: a `<item>_bar.png` (or a `bar_color` colormap) colours the durability bar, sampled with the remaining
  durability (1 = full) as `x`, `y`, `temperature` and `downfall`. The bar only shows on a damaged item.
- Custom particle types (`polytone/custom_particles/<name>.json`, drawn from `assets/<ns>/particles/<name>.json`): `initializer`
  (`size` is the quad size, `lifetime`, colour channels, `roll`, `friction`, `custom`, `customs`, `hitbox_size`, `dx`/`dy`/`dz`),
  `ticker` (`x`/`y`/`z`, `dx`/`dy`/`dz`, `size`, colour, `roll`, `custom`, `customs`, `remove_condition`, every `tick_interval`
  ticks; `o.custom()` and `o.custom('name')` read the variables), `has_physics`, `kill_on_contact`, `kill_when_still`,
  `liquid_affinity`, `light_level`, `random_sprite`, `render_type` (`invisible` draws nothing), `semi_custom` copying a vanilla
  type, and the particle's own `particle_emitters`. Spawned by `/particle ns:name`, by an emitter, or by block modifiers'
  `particle_emitters` (`animate_tick` source, `state_predicate` with `block_match`, `blockstate_match` and the random forms,
  `spawn_location` `center`/`lower_corner`, `chance`, `count`, `x`/`y`/`z` default to a 0.4 spread).
- Colormap `state_prop(i)` (position of the state's i-th property, by name order, over its value count) and `state_prop_i(i)`
  (the position itself) give a block tint one slot per distinct state.
- Colormap `color_modifier`: `red`/`green`/`blue` expressions over `RED`, `GREEN`, `BLUE` (0-255) rewrite the sampled colour;
  `legacy_temperature` samples like `temperature`.
- `fog_fade` (water and lava fog): the fog starts at `end - (end - start) * fade`, so 1 is vanilla and 0 is a hard edge.
  Block `map_color` (a `MapColor` name, with or without `map.`) recolours that block on filled maps.
- Particle modifiers (`particle_modifiers/`): `color`, `red/green/blue/alpha`, `size`, `life` and `speed` expressions run
  once when a particle is created, with `o`, `p`, `g` and `r` answered (no biome/light/colormap/config yet).
  `horizon_height` (the dark disc's height) and `water_fog_brightening_time` (ticks to full water vision) apply too.
- Colormap samplers `biome_id` (as Polytone: `1 - mapper.getIndex(biome)`; the mapper is the `biome_id_mapper` key, an
  inline object of biome ids to values with `texture_size`, or the name of a `biome_id_mappers/<name>.json`; none given
  uses Polytone's legacy OptiFine biome table, `(id + 1) / 256`; no biome gives 0), `item_damage`, `legacy_temperature`
  (cools above y 64, without the noise term) and the `type` shortcuts `grid`, `damage`, `fixed` read for block custom
  tints, item bars and the `colors.json` biome tables and water tint (not `legacy_temperature`, which needs a height).
- `config('ns:id')` answers in colormap axes and `color_modifier`, from the loaded pack's entries and the player's
  choices, alongside particle modifiers and conditions. A biome-table colormap that calls it is not read.
- Particle modifiers re-run colour and `size` (as a multiple of the spawn size) every tick on the particles they target;
  `life` and `speed` stay spawn-only.
- Block `can_occlude` (client side, as Polytone sets it on every state and rebuilds the cache): `false` shows the faces of
  neighbours behind the block and stops it being a solid render, `true` restores the shape's own occlusion. The client light
  engine's dampening is unchanged. `spawn_particles_on_break: false` removes a block's terrain particles (breaking, mining,
  landing) and `tinted_breaking_particles: false` leaves them untinted. Tag targets never match, as for block tints.
- Fluid modifiers targeting `minecraft:flowing_water` tint flowing water only; `minecraft:water` tints both unless a
  flowing one exists.
- Screen sprite modifiers apply `x_inc`/`y_inc`/`width_inc`/`height_inc` and `overlays` (drawn under the sprite, from
  its original box, tinted by the modifier colour) to `sprite`, `sprite_scaled`, `sprite_part`, `sprite_region`,
  `blit_sprite` and `nine_slice` draws.

## Next, in rough order

1. Custom `offset_type` objects.
3. Position-dependent colormaps for fluid tints (blocks and items already read the position).
4. Biome layers: `blend_to_gray`, `REMOVE`, colormap and expression arguments, other attributes; weather groups and the other dimension attributes; fog distances and cloud colour.
5. Custom particles still missing: GPU particles (`"type": "gpu"`), `model`, `rotation_mode`, `colormap`, `sound_emitters`,
   `exclusion_radius`/`limit`, `kill_when_not_in_view`, additive blending (`additive_translucent` draws as `translucent`),
   entity emitters, block emitters' `biomes`, `block_broken`/`block_cracking` tick sources and `tag_match` rule tests.

## Rules

- Tests: `cargo +stable test -p horizon-polytone` and
  `cargo +stable test -p horizon-client-render --lib biome` (the pinned 1.95.0 toolchain may not
  download here; stable's clippy flags unrelated `incompatible_msrv` and `unexpected_cfgs`).
- Pull before starting, push to `android` when done; small commits.
