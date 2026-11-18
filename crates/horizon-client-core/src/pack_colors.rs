use std::sync::RwLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DyeOverrides {
    pub diffuse: [Option<u32>; 16],
    pub text: [Option<u32>; 16],
    pub firework: [Option<u32>; 16],
    pub sheep: [Option<u32>; 16],
}

static OVERRIDES: RwLock<DyeOverrides> = RwLock::new(DyeOverrides {
    diffuse: [None; 16],
    text: [None; 16],
    firework: [None; 16],
    sheep: [None; 16],
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

pub fn sheep(index: usize) -> Option<u32> {
    read().sheep[index & 15]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::potions;

    #[test]
    fn overrides_replace_vanilla_and_clear_on_reset() {
        let mut o = DyeOverrides::default();
        o.diffuse[14] = Some(0x123456);
        o.sheep[3] = Some(0xABCDEF);
        o.text[0] = Some(0x010203);
        o.firework[5] = Some(0x0A0B0C);
        set(o);
        assert_eq!(dye(14, 0xB02E26), 0x123456);
        assert_eq!(dye(1, 0xF9801D), 0xF9801D);
        assert_eq!(sheep(3), Some(0xABCDEF));
        assert_eq!(sheep(4), None);
        assert_eq!(text(0, 0xFFFFFF), 0x010203);
        assert_eq!(text(1, 0xFF681F), 0xFF681F);
        assert_eq!(firework(5, 0x41CD34), 0x0A0B0C);
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
