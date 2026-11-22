//! The launcher's native-build chip: one word for "will my native mods be in
//! the game when I press Play?". Pure logic, so it is tested on the host; the
//! Android launcher only draws what `chip` returns.
use std::collections::BTreeSet;

/// What the launcher knows about the mods and the build on disk.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// Enabled native mods that need a compiled library (not run in-process).
    pub needed: BTreeSet<String>,
    /// Mod ids listed in the chosen build's `build.json`, when a build is chosen.
    pub built: Option<BTreeSet<String>>,
    /// A build exists and was made for this APK install.
    pub build_fits_apk: bool,
    /// A compile is running, with its progress 0..1.
    pub compiling: Option<f32>,
    /// The last compile's error, if it failed.
    pub failed: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Good,
    Warn,
    Bad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    Nothing,
    Compile,
    Retry,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub label: String,
    pub tone: Tone,
    pub tap: Tap,
}

fn chip(label: impl Into<String>, tone: Tone, tap: Tap) -> Chip {
    Chip { label: label.into(), tone, tap }
}

/// The chip for `s`, or `None` when no native mod needs a build (nothing to show).
pub fn chip_for(s: &Snapshot) -> Option<Chip> {
    if let Some(p) = s.compiling {
        let pct = (p.clamp(0.0, 1.0) * 100.0).round() as u32;
        return Some(chip(format!("Compiling {pct}%"), Tone::Neutral, Tap::Nothing));
    }
    if s.needed.is_empty() {
        return None;
    }
    if let Some(err) = &s.failed {
        let _ = err;
        return Some(chip("Compile failed - tap to retry", Tone::Bad, Tap::Retry));
    }
    let n = s.needed.len();
    let word = if n == 1 { "mod" } else { "mods" };
    match &s.built {
        None => Some(chip(format!("{n} {word} not compiled - tap to compile"), Tone::Warn, Tap::Compile)),
        Some(_) if !s.build_fits_apk => {
            Some(chip("Build is for an older install - tap to recompile", Tone::Warn, Tap::Compile))
        }
        Some(built) => {
            let missing = s.needed.iter().filter(|m| !built.contains(*m)).count();
            if missing == 0 {
                Some(chip(format!("{n} {word} compiled"), Tone::Good, Tap::Nothing))
            } else {
                Some(chip(format!("{missing} new - tap to recompile"), Tone::Warn, Tap::Compile))
            }
        }
    }
}

/// The mod ids a `build.json` lists (`"mods"` holds names or `{id, version}`).
pub fn built_ids(build_json: &str) -> BTreeSet<String> {
    let v: serde_json::Value = serde_json::from_str(build_json).unwrap_or_default();
    v["mods"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m.as_str().or_else(|| m["id"].as_str()).map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nothing_to_build_shows_nothing() {
        assert_eq!(chip_for(&Snapshot::default()), None);
    }

    #[test]
    fn uncompiled_mod_offers_compile() {
        let c = chip_for(&Snapshot { needed: set(&["wrench"]), ..Default::default() }).unwrap();
        assert_eq!(c.tap, Tap::Compile);
        assert!(c.label.contains("1 mod not compiled"));
    }

    #[test]
    fn compiled_set_is_good_and_idle() {
        let s = Snapshot {
            needed: set(&["wrench"]),
            built: Some(set(&["wrench", "x"])),
            build_fits_apk: true,
            ..Default::default()
        };
        let c = chip_for(&s).unwrap();
        assert_eq!((c.tone, c.tap), (Tone::Good, Tap::Nothing));
    }

    #[test]
    fn new_mod_or_new_apk_asks_for_recompile() {
        let mut s = Snapshot {
            needed: set(&["wrench", "new"]),
            built: Some(set(&["wrench"])),
            build_fits_apk: true,
            ..Default::default()
        };
        assert_eq!(chip_for(&s).unwrap().tap, Tap::Compile);
        s.needed = set(&["wrench"]);
        s.build_fits_apk = false;
        assert_eq!(chip_for(&s).unwrap().tap, Tap::Compile);
    }

    #[test]
    fn compiling_and_failure_win() {
        let s = Snapshot { needed: set(&["a"]), compiling: Some(0.456), ..Default::default() };
        assert_eq!(chip_for(&s).unwrap().label, "Compiling 46%");
        let s = Snapshot { needed: set(&["a"]), failed: Some("x".into()), ..Default::default() };
        assert_eq!(chip_for(&s).unwrap().tap, Tap::Retry);
    }

    #[test]
    fn build_json_ids() {
        let j = r#"{"mods":["a",{"id":"b","version":"1"}]}"#;
        assert_eq!(built_ids(j), set(&["a", "b"]));
        assert!(built_ids("garbage").is_empty());
    }
}
