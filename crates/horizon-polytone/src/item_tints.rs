use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::Modifiers;
use crate::colormaps::{Colormaps, SampleContext};
use crate::item_modifiers::{ItemModifier, Rarity, Tooltip};
use crate::tint::ColorSource;
pub use regex::Regex;

const TINT_SLOTS: i32 = 5;

pub struct ItemTints {
    set: Modifiers<ItemModifier>,
    maps: Colormaps,
    by_item: HashMap<String, usize>,
}

/// Whether a tooltip line shows a translation: the template's fixed text appears in it in
/// order, its placeholders (`%s`, `%1$s`) standing for anything (`Sharpness %s` shows as
/// `Sharpness V`, `Attack Damage` as `7 Attack Damage`).
fn shows_translation(plain: &str, translation: &str) -> bool {
    let mut segments = vec![String::new()];
    let mut chars = translation.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            segments.last_mut().unwrap().push(c);
        } else if chars.peek() == Some(&'%') {
            chars.next();
            segments.last_mut().unwrap().push('%');
        } else {
            for p in chars.by_ref() {
                if matches!(p, 's' | 'd' | 'f') {
                    break;
                }
            }
            segments.push(String::new());
        }
    }
    let segments: Vec<&str> = segments
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if segments.iter().map(|s| s.len()).sum::<usize>() < 3 {
        return false;
    }
    let mut rest = plain;
    for segment in segments {
        match rest.find(segment) {
            Some(at) => rest = &rest[at + segment.len()..],
            None => return false,
        }
    }
    true
}

/// An item's tooltip edited by its modifier: lines a `removed_tooltips` pattern matches are
/// dropped (`translations` are the texts of the keys the patterns match), then each added line
/// goes `position` lines below the first editable one (the end when it has none or it is past
/// the end; a negative one counts back from the end, `-1` being the end itself). The first
/// `keep` lines (the name) stay.
pub fn edit_tooltip<L>(
    lines: &mut Vec<L>,
    keep: usize,
    plain: &dyn Fn(&L) -> String,
    tooltips: &[Tooltip],
    removed: &[Regex],
    translations: &[String],
    make: &dyn Fn(&Tooltip) -> L,
) {
    if !removed.is_empty() {
        let mut index = 0;
        lines.retain(|line| {
            index += 1;
            if index <= keep {
                return true;
            }
            let text = plain(line);
            let gone = removed.iter().any(|pattern| pattern.is_match(&text))
                || translations.iter().any(|t| shows_translation(&text, t));
            !gone
        });
    }
    for tooltip in tooltips {
        let len = lines.len();
        let at = match tooltip.position {
            None => len,
            Some(p) if p >= 0 => (keep + p as usize).min(len),
            Some(p) => (len as i64 + i64::from(p) + 1).clamp(keep as i64, len as i64) as usize,
        };
        lines.insert(at, make(tooltip));
    }
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
                && set.get(index).is_some_and(|m| {
                    m.tint.is_some()
                        || m.bar.is_some()
                        || m.rarity.is_some()
                        || !m.tooltips.is_empty()
                        || !m.removed_tooltips.is_empty()
                })
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

    fn modifier(&self, item: &str) -> Option<&ItemModifier> {
        self.by_item.get(item).and_then(|i| self.set.get(*i))
    }

    pub fn bar_color(&self, item: &str) -> Option<u32> {
        match self.modifier(item)?.bar.as_ref()?.source(0)? {
            ColorSource::Flat(color) => Some(*color & 0xFF_FFFF),
            _ => None,
        }
    }

    pub fn tooltip_edits(&self, item: &str) -> Option<(&[Tooltip], &[Regex])> {
        let m = self.modifier(item)?;
        (!m.tooltips.is_empty() || !m.removed_tooltips.is_empty())
            .then_some((m.tooltips.as_slice(), m.removed_tooltips.as_slice()))
    }

    pub fn rarity(&self, item: &str) -> Option<&'static str> {
        Some(match self.modifier(item)?.rarity? {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
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
    fn a_flat_bar_colour_and_a_rarity_are_read_and_a_textured_bar_is_left_to_vanilla() {
        let root = scratch("item-bar");
        put(
            &root,
            "assets/foo/polytone/item_modifiers/pick.json",
            r##"{"bar_color": "#00FFFF", "rarity": "epic"}"##,
        );
        put(
            &root,
            "assets/foo/polytone/item_modifiers/axe.json",
            r##"{"bar_color": "damage"}"##,
        );
        let items = ItemTints::read(
            &root,
            [
                "foo:pick".to_owned(),
                "foo:axe".to_owned(),
                "foo:none".to_owned(),
            ]
            .into_iter(),
            &none,
        )
        .unwrap();
        assert_eq!(items.bar_color("foo:pick"), Some(0x00FFFF));
        assert_eq!(items.rarity("foo:pick"), Some("epic"));
        assert_eq!(items.bar_color("foo:axe"), None);
        assert_eq!(items.rarity("foo:axe"), None);
        assert_eq!(items.bar_color("foo:none"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pack_without_item_tints_gives_nothing() {
        let root = scratch("item-tints-empty");
        assert!(ItemTints::read(&root, ["foo:stick".to_owned()].into_iter(), &none).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod tooltip_tests {
    use super::*;

    fn tip(text: &str, position: Option<i32>) -> Tooltip {
        Tooltip {
            text: Some(text.to_owned()),
            component: None,
            color: None,
            bold: None,
            italic: None,
            position,
        }
    }

    fn edit(
        lines: &[&str],
        tips: &[Tooltip],
        removed: &[&str],
        translations: &[&str],
    ) -> Vec<String> {
        let mut lines: Vec<String> = lines.iter().map(|s| (*s).to_owned()).collect();
        let removed: Vec<Regex> = removed.iter().map(|r| Regex::new(r).unwrap()).collect();
        let translations: Vec<String> = translations.iter().map(|s| (*s).to_owned()).collect();
        edit_tooltip(
            &mut lines,
            1,
            &|l| l.clone(),
            tips,
            &removed,
            &translations,
            &|t| t.text.clone().unwrap(),
        );
        lines
    }

    #[test]
    fn added_lines_go_to_their_position_and_default_to_the_end() {
        let base = ["Sword", "Sharpness V", "Durability"];
        assert_eq!(
            edit(&base, &[tip("A", Some(0))], &[], &[]),
            ["Sword", "A", "Sharpness V", "Durability"]
        );
        assert_eq!(
            edit(&base, &[tip("A", Some(1))], &[], &[]),
            ["Sword", "Sharpness V", "A", "Durability"]
        );
        assert_eq!(
            edit(&base, &[tip("A", None), tip("B", Some(99))], &[], &[]),
            ["Sword", "Sharpness V", "Durability", "A", "B"]
        );
        assert_eq!(
            edit(&base, &[tip("A", Some(-1))], &[], &[]),
            ["Sword", "Sharpness V", "Durability", "A"]
        );
        assert_eq!(
            edit(&base, &[tip("A", Some(-2))], &[], &[]),
            ["Sword", "Sharpness V", "A", "Durability"]
        );
        assert_eq!(
            edit(&base, &[tip("A", Some(-99))], &[], &[]),
            ["Sword", "A", "Sharpness V", "Durability"]
        );
    }

    #[test]
    fn removed_lines_match_by_text_or_by_the_translation_of_a_matched_key() {
        let base = [
            "Sword",
            "Sharpness V",
            "When in Main Hand:",
            "Durability: 5 / 9",
        ];
        assert_eq!(
            edit(&base, &[], &["^Durab"], &[]),
            ["Sword", "Sharpness V", "When in Main Hand:"]
        );
        assert_eq!(
            edit(&base, &[], &["enchantment\\..*"], &["Sharpness %s"]),
            ["Sword", "When in Main Hand:", "Durability: 5 / 9"]
        );
        assert_eq!(
            edit(&base, &[], &["item\\.modifiers.*"], &["When in Main Hand:"]),
            ["Sword", "Sharpness V", "Durability: 5 / 9"]
        );
        assert_eq!(edit(&base, &[], &["Sword"], &[]), base);
        let stats = ["Sword", " 7 Attack Damage", " 1.6 Attack Speed"];
        assert_eq!(
            edit(
                &stats,
                &[],
                &["attribute\\.name\\.attack_damage"],
                &["Attack Damage"]
            ),
            ["Sword", " 1.6 Attack Speed"]
        );
        assert_eq!(edit(&stats, &[], &["x"], &["+%s %s", "%s%%"]), stats);
    }
}
