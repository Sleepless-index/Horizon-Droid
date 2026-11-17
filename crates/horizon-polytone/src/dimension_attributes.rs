use std::path::Path;

use crate::attributes::{Argument, Attributes, Layer, Modifier};
use crate::biome_attributes::Channel;
use crate::files;
use crate::modifiers::DimensionModifier;
use crate::set::Modifiers;

#[derive(Debug, Default)]
pub struct DimensionAttributes {
    modifiers: Modifiers<DimensionModifier>,
}

#[derive(Debug, Default, Clone)]
pub struct DimensionEdits {
    base: Attributes,
    post_process: Attributes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Base,
    PostProcess,
}

impl DimensionAttributes {
    pub fn read(root: &Path) -> Self {
        let mut modifiers = Modifiers::default();
        for entry in files::scan(root, "dimension_modifiers") {
            let Some(json) = &entry.json else { continue };
            match DimensionModifier::parse(json, entry.id.as_deref()) {
                Ok(Some(m)) => modifiers.push(m.targets.clone(), m.priority, m),
                Ok(None) => {}
                Err(e) => log::warn!("{}: {e}", entry.label),
            }
        }
        Self { modifiers }
    }

    pub fn edits(&self, dimension: &str) -> DimensionEdits {
        match self.modifiers.resolve(dimension, &|_| false) {
            Some(m) => DimensionEdits {
                base: m.base.clone(),
                post_process: m.post_process.clone(),
            },
            None => DimensionEdits::default(),
        }
    }
}

impl DimensionEdits {
    pub fn is_empty(&self) -> bool {
        self.base.is_empty() && self.post_process.is_empty()
    }

    fn layer(&self, group: Group, id: &str) -> Option<&Layer> {
        match group {
            Group::Base => &self.base,
            Group::PostProcess => &self.post_process,
        }
        .get(id)
    }

    pub fn color(&self, group: Group, id: &str, vanilla: [f32; 3]) -> [f32; 3] {
        let Some(Layer::Apply {
            modifier, argument, ..
        }) = self.layer(group, id)
        else {
            return vanilla;
        };
        let mut channel = Channel::default();
        channel.push(*modifier, argument);
        let pack = vanilla.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u32);
        let packed = pack[0] << 16 | pack[1] << 8 | pack[2];
        match channel.resolve(Some(packed)) {
            Some(c) => [
                (c >> 16 & 255) as f32 / 255.0,
                (c >> 8 & 255) as f32 / 255.0,
                (c & 255) as f32 / 255.0,
            ],
            None => vanilla,
        }
    }

    pub fn scalar(&self, group: Group, id: &str, vanilla: f32) -> f32 {
        let Some(Layer::Apply {
            modifier,
            argument: Argument::Number(n),
            ..
        }) = self.layer(group, id)
        else {
            return vanilla;
        };
        let n = *n as f32;
        match modifier {
            Modifier::Override => n,
            Modifier::Add => vanilla + n,
            Modifier::Subtract => vanilla - n,
            Modifier::Multiply => vanilla * n,
            Modifier::Minimum => vanilla.min(n),
            Modifier::Maximum => vanilla.max(n),
            _ => vanilla,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::testing::{put, scratch};

    #[test]
    fn a_dimension_modifier_edits_colours_and_distances_of_its_target() {
        let root = scratch("dim-attrs");
        put(
            &root,
            "assets/foo/polytone/dimension_modifiers/nether.json",
            r##"{"targets": "minecraft:the_nether", "attributes_modifiers": {
                "base": {"visual/fog_end_distance": {"argument": 0.5, "modifier": "multiply"}},
                "post_process": {"visual/fog_color": "#102030",
                                 "visual/cloud_color": {"argument": "#101010", "modifier": "add"}}}}"##,
        );
        let attrs = DimensionAttributes::read(&root);
        let nether = attrs.edits("minecraft:the_nether");
        assert_eq!(
            nether.scalar(Group::Base, "minecraft:visual/fog_end_distance", 96.0),
            48.0
        );
        assert_eq!(
            nether.scalar(
                Group::PostProcess,
                "minecraft:visual/fog_end_distance",
                96.0
            ),
            96.0
        );
        let fog = nether.color(
            Group::PostProcess,
            "minecraft:visual/fog_color",
            [1.0, 1.0, 1.0],
        );
        assert_eq!(fog, [16.0 / 255.0, 32.0 / 255.0, 48.0 / 255.0]);
        let cloud = nether.color(Group::PostProcess, "minecraft:visual/cloud_color", [0.0; 3]);
        assert_eq!(cloud, [16.0 / 255.0; 3]);
        assert!(attrs.edits("minecraft:overworld").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
