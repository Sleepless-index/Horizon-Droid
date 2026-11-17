use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::Modifiers;
use crate::colormaps::{Colormaps, SampleContext};
use crate::item_modifiers::ItemModifier;

const TINT_SLOTS: i32 = 5;

pub struct ItemTints {
    set: Modifiers<ItemModifier>,
    maps: Colormaps,
    by_item: HashMap<String, usize>,
}

impl ItemTints {
    pub fn read(
        root: &Path,
        items: impl Iterator<Item = String>,
        tags: &dyn Fn() -> HashMap<String, HashSet<String>>,
    ) -> Option<Self> {
        let set = ItemModifier::read(root);
        if set.is_empty() {
            return None;
        }
        let tags = tags();
        let mut by_item = HashMap::new();
        for item in items {
            let in_tag = |tag: &str| tags.get(tag).is_some_and(|members| members.contains(&item));
            if let Some(index) = set.resolve_index(&item, &in_tag)
                && set.get(index).is_some_and(|m| m.tint.is_some())
            {
                by_item.insert(item, index);
            }
        }
        if by_item.is_empty() {
            return None;
        }
        Some(Self {
            set,
            maps: Colormaps::read(root),
            by_item,
        })
    }

    pub fn retint(&self, item: &str, at: [f64; 3], tints: &mut Vec<i32>, opaque: fn(i32) -> i32) {
        let Some(tint) = self
            .by_item
            .get(item)
            .and_then(|i| self.set.get(*i))
            .and_then(|m| m.tint.as_ref())
        else {
            return;
        };
        let context = SampleContext {
            x: at[0],
            y: at[1],
            z: at[2],
            temperature: 0.8,
            downfall: 0.4,
        };
        for index in 0..TINT_SLOTS {
            let Some(color) = tint.color(index, &self.maps, &context) else {
                continue;
            };
            if tints.len() <= index as usize {
                tints.resize(index as usize + 1, -1);
            }
            tints[index as usize] = opaque(color as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::testing::*;

    fn opaque(rgb: i32) -> i32 {
        rgb | 0xFF00_0000_u32 as i32
    }

    fn none() -> HashMap<String, HashSet<String>> {
        HashMap::new()
    }

    #[test]
    fn a_tint_replaces_the_slots_it_sets_and_leaves_the_rest() {
        let root = scratch("item-tints");
        put(
            &root,
            "assets/foo/polytone/item_modifiers/stick.json",
            r##"{"colormap": {"1": "#336699"}}"##,
        );
        let items = ItemTints::read(
            &root,
            ["foo:stick".to_owned(), "foo:other".to_owned()].into_iter(),
            &none,
        )
        .unwrap();
        let mut tints = vec![0x11223344, 0x55667788];
        items.retint("foo:stick", [0.0; 3], &mut tints, opaque);
        assert_eq!(tints, vec![0x11223344, 0xFF336699_u32 as i32]);
        let mut other = vec![0x11223344];
        items.retint("foo:other", [0.0; 3], &mut other, opaque);
        assert_eq!(other, vec![0x11223344]);
        let mut short = vec![0x11223344];
        items.retint("foo:stick", [0.0; 3], &mut short, opaque);
        assert_eq!(short, vec![0x11223344, 0xFF336699_u32 as i32]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pack_without_item_tints_gives_nothing() {
        let root = scratch("item-tints-empty");
        assert!(ItemTints::read(&root, ["foo:stick".to_owned()].into_iter(), &none).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
