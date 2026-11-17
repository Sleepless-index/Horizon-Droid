use std::path::Path;

use crate::attributes::{Argument, Attributes, Layer, Modifier};
use crate::files;
use crate::modifiers::BiomeModifier;
use crate::set::Modifiers;

const SKY: &str = "minecraft:visual/sky_color";
const FOG: &str = "minecraft:visual/fog_color";
const WATER_FOG: &str = "minecraft:visual/water_fog_color";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Operand {
    Argb(u32),
    Scale(f64),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Channel {
    ops: Vec<(Modifier, Operand)>,
}

impl Channel {
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn resolve(&self, vanilla: Option<u32>) -> Option<u32> {
        let mut value = vanilla;
        for (modifier, operand) in &self.ops {
            value = match (modifier, operand) {
                (Modifier::Override, Operand::Argb(c)) => Some(c & 0xFF_FFFF),
                (_, operand) => value.and_then(|v| modify(v, *modifier, *operand)),
            }
            .or(value);
        }
        value
    }

    pub fn push(&mut self, modifier: Modifier, argument: &Argument) {
        let operand = match (modifier, argument) {
            (Modifier::Override, Argument::Color(argb)) => Operand::Argb(*argb),
            (Modifier::Override, Argument::Number(n)) if n.is_finite() => {
                Operand::Argb(*n as i64 as u32)
            }
            (Modifier::Multiply, Argument::Number(n)) if n.is_finite() => Operand::Scale(*n),
            (Modifier::Override, _) => return,
            (_, Argument::Color(argb)) => Operand::Argb(*argb),
            _ => return,
        };
        self.ops.push((modifier, operand));
    }
}

fn modify(base: u32, modifier: Modifier, operand: Operand) -> Option<u32> {
    let channels = |c: u32| [(c >> 16) & 255, (c >> 8) & 255, c & 255];
    let pack = |c: [u32; 3]| c[0] << 16 | c[1] << 8 | c[2];
    let b = channels(base);
    let combine = |f: &dyn Fn(u32, u32) -> u32, argb: u32| {
        let a = channels(argb);
        pack([f(b[0], a[0]), f(b[1], a[1]), f(b[2], a[2])])
    };
    Some(match (modifier, operand) {
        (Modifier::Multiply, Operand::Scale(k)) => {
            pack(b.map(|c| (f64::from(c) * k).round().clamp(0.0, 255.0) as u32))
        }
        (Modifier::Add, Operand::Argb(c)) => combine(&|x, y| (x + y).min(255), c),
        (Modifier::Subtract, Operand::Argb(c)) => combine(&|x, y| x.saturating_sub(y), c),
        (Modifier::Multiply, Operand::Argb(c)) => combine(&|x, y| x * y / 255, c),
        (Modifier::Minimum, Operand::Argb(c)) => combine(&|x, y| x.min(y), c),
        (Modifier::Maximum, Operand::Argb(c)) => combine(&|x, y| x.max(y), c),
        (Modifier::AlphaBlend, Operand::Argb(c)) => {
            let alpha = (c >> 24) & 255;
            combine(&|x, y| (x * (255 - alpha) + y * alpha + 127) / 255, c)
        }
        _ => return None,
    })
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BiomeVisuals {
    pub sky: Channel,
    pub fog: Channel,
    pub water_fog: Channel,
}

impl BiomeVisuals {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn apply(&mut self, attributes: &Attributes) {
        for (channel, id) in [
            (&mut self.sky, SKY),
            (&mut self.fog, FOG),
            (&mut self.water_fog, WATER_FOG),
        ] {
            if let Some(Layer::Apply {
                modifier, argument, ..
            }) = attributes.get(id)
            {
                channel.push(*modifier, argument);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct BiomeAttributes {
    modifiers: Modifiers<BiomeModifier>,
}

impl BiomeAttributes {
    pub fn read(root: &Path) -> Self {
        let mut modifiers = Modifiers::default();
        for entry in files::scan(root, "biome_modifiers") {
            let Some(json) = &entry.json else { continue };
            match BiomeModifier::parse(json, entry.id.as_deref()) {
                Ok(Some(m)) => modifiers.push(m.targets.clone(), m.priority, m),
                Ok(None) => {}
                Err(e) => log::warn!("{}: {e}", entry.label),
            }
        }
        Self { modifiers }
    }

    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty()
    }

    pub fn visuals(&self, biome: &str, in_tag: &dyn Fn(&str) -> bool) -> BiomeVisuals {
        let mut visuals = BiomeVisuals::default();
        if let Some(m) = self.modifiers.resolve(biome, in_tag) {
            visuals.apply(&m.base);
            visuals.apply(&m.post_process);
        }
        visuals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::testing::{put, scratch};

    #[test]
    fn a_biome_modifier_overrides_sky_fog_and_water_fog_of_its_target() {
        let root = scratch("biome-attrs");
        put(
            &root,
            "assets/foo/polytone/biome_modifiers/swamp.json",
            r##"{"targets": "minecraft:swamp", "attributes_modifiers": {
                "visual/sky_color": "#102030", "visual/fog_color": 255, "visual/water_fog_color": "#ff010203"}}"##,
        );
        let attrs = BiomeAttributes::read(&root);
        assert!(!attrs.is_empty());
        let swamp = attrs.visuals("minecraft:swamp", &|_| false);
        assert_eq!(swamp.sky.resolve(None), Some(0x102030));
        assert_eq!(swamp.fog.resolve(None), Some(0x0000FF));
        assert_eq!(swamp.water_fog.resolve(None), Some(0x010203));
        assert!(attrs.visuals("minecraft:plains", &|_| false).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_tag_target_reaches_the_biomes_in_the_tag() {
        let root = scratch("biome-attrs-tag");
        put(
            &root,
            "assets/foo/polytone/biome_modifiers/oceans.json",
            r##"{"targets": "#minecraft:is_ocean", "attributes_modifiers": {"visual/sky_color": "#0A0B0C"}}"##,
        );
        let attrs = BiomeAttributes::read(&root);
        let in_tag = |tag: &str| tag == "minecraft:is_ocean";
        assert_eq!(
            attrs.visuals("minecraft:ocean", &in_tag).sky.resolve(None),
            Some(0x0A0B0C)
        );
        assert!(attrs.visuals("minecraft:ocean", &|_| false).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn colour_modifiers_edit_the_colour_the_biome_has_in_the_order_the_layers_come() {
        let root = scratch("biome-attrs-ops");
        put(
            &root,
            "assets/foo/polytone/biome_modifiers/ops.json",
            r##"{"targets": "minecraft:swamp", "attributes_modifiers": {
                "base": {"visual/sky_color": {"argument": "#102030", "modifier": "add"},
                         "visual/fog_color": {"argument": "#80ff0000", "modifier": "alpha_blend"},
                         "visual/water_fog_color": {"argument": "#0A0A0A", "modifier": "subtract"}}}}"##,
        );
        let swamp = BiomeAttributes::read(&root).visuals("minecraft:swamp", &|_| false);
        assert_eq!(swamp.sky.resolve(Some(0xF0_1010)), Some(0xFF_3040));
        assert_eq!(swamp.fog.resolve(Some(0x00_0000)), Some(0x80_0000));
        assert_eq!(swamp.water_fog.resolve(Some(0x05_0533)), Some(0x00_0029));
        assert_eq!(swamp.sky.resolve(None), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn post_process_wins_over_base_and_modifying_layers_change_the_biomes_own_colour() {
        let root = scratch("biome-attrs-post");
        put(
            &root,
            "assets/foo/polytone/biome_modifiers/plains.json",
            r##"{"targets": "minecraft:plains", "attributes_modifiers": {
                "base": {"visual/sky_color": "#111111", "visual/fog_color": {"argument": 0.5, "modifier": "multiply"}},
                "post_process": {"visual/sky_color": "#222222"}}}"##,
        );
        let attrs = BiomeAttributes::read(&root);
        assert!(attrs.visuals("foo:plains", &|_| false).is_empty());
        let plains = attrs.visuals("minecraft:plains", &|_| false);
        assert_eq!(plains.sky.resolve(Some(0x999999)), Some(0x222222));
        assert_eq!(plains.fog.resolve(None), None);
        assert_eq!(plains.fog.resolve(Some(0x80_6040)), Some(0x40_3020));
        let _ = std::fs::remove_dir_all(&root);
    }
}
