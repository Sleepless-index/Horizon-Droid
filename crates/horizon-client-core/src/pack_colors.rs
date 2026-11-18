use std::sync::RwLock;

#[derive(Clone, Copy, Default)]
struct Overrides {
    dye: [Option<u32>; 16],
    sheep: [Option<u32>; 16],
}

static OVERRIDES: RwLock<Overrides> = RwLock::new(Overrides {
    dye: [None; 16],
    sheep: [None; 16],
});

fn read() -> Overrides {
    OVERRIDES.read().map(|guard| *guard).unwrap_or_default()
}

pub fn set(dye: [Option<u32>; 16], sheep: [Option<u32>; 16]) {
    if let Ok(mut guard) = OVERRIDES.write() {
        *guard = Overrides { dye, sheep };
    }
}

pub fn dye(index: usize, vanilla: u32) -> u32 {
    read().dye[index & 15].unwrap_or(vanilla)
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
        let mut dye = [None; 16];
        dye[14] = Some(0x123456);
        let mut wool = [None; 16];
        wool[3] = Some(0xABCDEF);
        set(dye, wool);
        assert_eq!(super::dye(14, 0xB02E26), 0x123456);
        assert_eq!(super::dye(1, 0xF9801D), 0xF9801D);
        assert_eq!(sheep(3), Some(0xABCDEF));
        assert_eq!(sheep(4), None);
        set([None; 16], [None; 16]);
        assert_eq!(super::dye(14, 0xB02E26), 0xB02E26);

        potions::set_pack_effect_colors(&[("speed".into(), 0xFF0000FF)]);
        assert_eq!(potions::effect("minecraft:speed").unwrap().color, 0x0000FF);
        assert_eq!(potions::effect("poison").unwrap().color, 8889187);
        potions::set_pack_effect_colors(&[]);
        assert_ne!(potions::effect("speed").unwrap().color, 0x0000FF);
    }
}
