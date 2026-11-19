use std::collections::HashMap;
use std::sync::RwLock;

pub const MAP_COLOR_NAMES: [&str; 62] = [
    "none",
    "grass",
    "sand",
    "wool",
    "fire",
    "ice",
    "metal",
    "plant",
    "snow",
    "clay",
    "dirt",
    "stone",
    "water",
    "wood",
    "quartz",
    "color_orange",
    "color_magenta",
    "color_light_blue",
    "color_yellow",
    "color_light_green",
    "color_pink",
    "color_gray",
    "color_light_gray",
    "color_cyan",
    "color_purple",
    "color_blue",
    "color_brown",
    "color_green",
    "color_red",
    "color_black",
    "gold",
    "diamond",
    "lapis",
    "emerald",
    "podzol",
    "nether",
    "terracotta_white",
    "terracotta_orange",
    "terracotta_magenta",
    "terracotta_light_blue",
    "terracotta_yellow",
    "terracotta_light_green",
    "terracotta_pink",
    "terracotta_gray",
    "terracotta_light_gray",
    "terracotta_cyan",
    "terracotta_purple",
    "terracotta_blue",
    "terracotta_brown",
    "terracotta_green",
    "terracotta_red",
    "terracotta_black",
    "crimson_nylium",
    "crimson_stem",
    "crimson_hyphae",
    "warped_nylium",
    "warped_stem",
    "warped_hyphae",
    "warped_wart_block",
    "deepslate",
    "raw_iron",
    "glow_lichen",
];

static MAP_COLORS: RwLock<Option<HashMap<u32, u8>>> = RwLock::new(None);

pub fn map_color_id(name: &str) -> Option<u8> {
    let name = name.trim().trim_start_matches("map.").to_ascii_lowercase();
    MAP_COLOR_NAMES
        .iter()
        .position(|n| *n == name)
        .map(|i| i as u8)
}

pub fn set_map_colors(colors: HashMap<u32, u8>) {
    if let Ok(mut guard) = MAP_COLORS.write() {
        *guard = (!colors.is_empty()).then_some(colors);
    }
}

pub fn map_color(state: u32, vanilla: u8) -> u8 {
    MAP_COLORS
        .read()
        .ok()
        .and_then(|guard| guard.as_ref().and_then(|m| m.get(&state).copied()))
        .unwrap_or(vanilla)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DyeOverrides {
    pub diffuse: [Option<u32>; 16],
    pub text: [Option<u32>; 16],
    pub firework: [Option<u32>; 16],
    pub sheep: [Option<u32>; 16],
    pub redstone: [Option<u32>; 16],
}

static OVERRIDES: RwLock<DyeOverrides> = RwLock::new(DyeOverrides {
    diffuse: [None; 16],
    text: [None; 16],
    firework: [None; 16],
    sheep: [None; 16],
    redstone: [None; 16],
});

fn read() -> DyeOverrides {
    OVERRIDES.read().map(|guard| *guard).unwrap_or_default()
}

pub fn set(overrides: DyeOverrides) {
    if let Ok(mut guard) = OVERRIDES.write() {
        *guard = overrides;
    }
}

pub fn dye(index: usize, vanilla: u32) -> u32 {
    read().diffuse[index & 15].unwrap_or(vanilla)
}

pub fn text(index: usize, vanilla: u32) -> u32 {
    read().text[index & 15].unwrap_or(vanilla)
}

pub fn firework(index: usize, vanilla: u32) -> u32 {
    read().firework[index & 15].unwrap_or(vanilla)
}

pub fn redstone(power: u8) -> Option<u32> {
    read().redstone[usize::from(power & 15)]
}

pub fn sheep(index: usize) -> Option<u32> {
    read().sheep[index & 15]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::potions;

    #[test]
    fn map_colors_follow_vanilla_ids_and_override_by_state() {
        assert_eq!(map_color_id("plant"), Some(7));
        assert_eq!(map_color_id("map.COLOR_RED"), Some(28));
        assert_eq!(map_color_id("glow_lichen"), Some(61));
        assert_eq!(map_color_id("teal"), None);
        set_map_colors(HashMap::from([(5, 12)]));
        assert_eq!(map_color(5, 1), 12);
        assert_eq!(map_color(6, 1), 1);
        set_map_colors(HashMap::new());
        assert_eq!(map_color(5, 1), 1);
    }

    #[test]
    fn overrides_replace_vanilla_and_clear_on_reset() {
        let mut o = DyeOverrides::default();
        o.diffuse[14] = Some(0x123456);
        o.sheep[3] = Some(0xABCDEF);
        o.text[0] = Some(0x010203);
        o.firework[5] = Some(0x0A0B0C);
        o.redstone[15] = Some(0x00FFFF);
        set(o);
        assert_eq!(dye(14, 0xB02E26), 0x123456);
        assert_eq!(dye(1, 0xF9801D), 0xF9801D);
        assert_eq!(sheep(3), Some(0xABCDEF));
        assert_eq!(sheep(4), None);
        assert_eq!(text(0, 0xFFFFFF), 0x010203);
        assert_eq!(text(1, 0xFF681F), 0xFF681F);
        assert_eq!(firework(5, 0x41CD34), 0x0A0B0C);
        assert_eq!(redstone(15), Some(0x00FFFF));
        assert_eq!(redstone(14), None);
        assert_eq!(firework(6, 0xD88198), 0xD88198);
        set(DyeOverrides::default());
        assert_eq!(dye(14, 0xB02E26), 0xB02E26);
        assert_eq!(text(0, 0xFFFFFF), 0xFFFFFF);

        potions::set_pack_effect_colors(&[("speed".into(), 0xFF0000FF)]);
        assert_eq!(potions::effect("minecraft:speed").unwrap().color, 0x0000FF);
        assert_eq!(potions::effect("poison").unwrap().color, 8889187);
        potions::set_pack_effect_colors(&[]);
        assert_ne!(potions::effect("speed").unwrap().color, 0x0000FF);
    }
}
