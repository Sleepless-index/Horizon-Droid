use crate::lang;
use horizon_polytone::pack::{ConfigKind, ConfigValue, Configs};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct PolytoneConfig {
    configs: Configs,
    order: Vec<String>,
    preset: Option<usize>,
    pub changed: bool,
}

impl PolytoneConfig {
    pub fn new(configs: Configs) -> Self {
        let mut order: Vec<String> = configs.entries().keys().cloned().collect();
        order.sort_by_key(|id| {
            let e = &configs.entries()[id];
            (
                e.section_order.unwrap_or(i64::MAX),
                e.section.clone(),
                e.display_order,
                id.clone(),
            )
        });
        Self {
            configs,
            order,
            preset: None,
            changed: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn ids(&self) -> &[String] {
        &self.order
    }

    pub fn label(&self, id: &str) -> String {
        let (namespace, name) = id.split_once(':').unwrap_or(("minecraft", id));
        let key = format!("config.{namespace}.{name}");
        if lang::has(&key) {
            return lang::get(&key);
        }
        name.rsplit('/').next().unwrap_or(name).replace('_', " ")
    }

    pub fn value_text(&self, id: &str) -> String {
        match self.configs.get(id) {
            Some(ConfigValue::Bool(true)) => lang::get("options.on"),
            Some(ConfigValue::Bool(false)) => lang::get("options.off"),
            Some(ConfigValue::Number(n)) => format!("{}", (n * 1000.0).round() / 1000.0),
            Some(ConfigValue::Text(t)) => lang::get_or(t, t.trim_start_matches('§')),
            None => String::new(),
        }
    }

    pub fn button_label(&self, id: &str) -> String {
        format!("{}: {}", self.label(id), self.value_text(id))
    }

    pub fn value(&self, id: &str) -> Option<&ConfigValue> {
        self.configs.get(id)
    }

    pub fn step(&mut self, id: &str, backwards: bool) {
        let (Some(entry), Some(current)) = (self.configs.entries().get(id), self.configs.get(id))
        else {
            return;
        };
        let next = match (&entry.kind, current) {
            (ConfigKind::Bool, ConfigValue::Bool(b)) => ConfigValue::Bool(!b),
            (ConfigKind::Number { min, max, step }, ConfigValue::Number(n)) => {
                let step = if *step > 0.0 { *step } else { 0.1 };
                let moved = if backwards { n - step } else { n + step };
                let wrapped = if moved > *max + step * 1e-6 {
                    *min
                } else if moved < *min - step * 1e-6 {
                    *max
                } else {
                    moved
                };
                ConfigValue::Number((wrapped * 1e6).round() / 1e6)
            }
            (ConfigKind::Text { allowed }, ConfigValue::Text(t)) if !allowed.is_empty() => {
                let at = allowed.iter().position(|a| a == t).unwrap_or(0);
                let len = allowed.len();
                let at = if backwards {
                    (at + len - 1) % len
                } else {
                    (at + 1) % len
                };
                ConfigValue::Text(allowed[at].clone())
            }
            _ => return,
        };
        self.configs.set(id, next);
        self.preset = None;
        self.changed = true;
    }

    pub fn reset(&mut self, id: &str) {
        if let Some(entry) = self.configs.entries().get(id) {
            let default = entry.default.clone();
            self.configs.set(id, default);
            self.preset = None;
            self.changed = true;
        }
    }

    pub fn reset_all(&mut self) {
        for id in self.order.clone() {
            self.reset(&id);
        }
    }

    pub fn has_presets(&self) -> bool {
        !self.configs.presets().is_empty()
    }

    pub fn preset_label(&self) -> String {
        let names = self.configs.presets();
        let name = self.preset.and_then(|i| names.get(i)).map_or_else(
            || lang::get_or("horizon.polytone_config.custom", "Custom"),
            Clone::clone,
        );
        format!(
            "{}: {name}",
            lang::get_or("horizon.polytone_config.preset", "Preset")
        )
    }

    pub fn next_preset(&mut self) {
        let names = self.configs.presets();
        if names.is_empty() {
            return;
        }
        let next = self.preset.map_or(0, |i| (i + 1) % names.len());
        self.configs.apply_preset(&names[next]);
        self.preset = Some(next);
        self.changed = true;
    }

    pub fn chosen(&self) -> BTreeMap<String, ConfigValue> {
        self.configs.chosen()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> PolytoneConfig {
        let mut configs = Configs::default();
        configs.add_entry(
            "p:fancy".into(),
            &json!({"default_value": false, "presets": {"low": false, "high": true}}),
        );
        configs.add_entry(
            "p:fog".into(),
            &json!({"default_value": 60, "min": 0, "max": 100, "step": 20, "presets": {"low": 0, "high": 100}}),
        );
        configs.add_entry(
            "p:mode".into(),
            &json!({"default_value": "a", "allowed_values": ["a", "b", "c"]}),
        );
        PolytoneConfig::new(configs)
    }

    #[test]
    fn stepping_toggles_wraps_and_cycles() {
        let mut c = config();
        assert!(!c.changed);
        c.step("p:fancy", false);
        assert_eq!(c.value("p:fancy"), Some(&ConfigValue::Bool(true)));
        c.step("p:fog", false);
        assert_eq!(c.value("p:fog"), Some(&ConfigValue::Number(80.0)));
        c.step("p:fog", false);
        c.step("p:fog", false);
        assert_eq!(c.value("p:fog"), Some(&ConfigValue::Number(0.0)));
        c.step("p:fog", true);
        assert_eq!(c.value("p:fog"), Some(&ConfigValue::Number(100.0)));
        c.step("p:mode", true);
        assert_eq!(c.value("p:mode"), Some(&ConfigValue::Text("c".into())));
        c.step("p:mode", false);
        assert_eq!(c.value("p:mode"), Some(&ConfigValue::Text("a".into())));
        assert!(c.changed);
    }

    #[test]
    fn only_values_that_differ_from_the_defaults_are_chosen() {
        let mut c = config();
        c.step("p:fancy", false);
        c.step("p:mode", false);
        c.step("p:mode", true);
        let chosen = c.chosen();
        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen["p:fancy"], ConfigValue::Bool(true));
        c.reset_all();
        assert!(c.chosen().is_empty());
    }

    #[test]
    fn presets_cycle_and_a_manual_change_leaves_them() {
        let mut c = config();
        assert!(c.has_presets());
        c.next_preset();
        assert_eq!(c.value("p:fancy"), Some(&ConfigValue::Bool(true)));
        assert_eq!(c.value("p:fog"), Some(&ConfigValue::Number(100.0)));
        c.next_preset();
        assert_eq!(c.value("p:fancy"), Some(&ConfigValue::Bool(false)));
        assert_eq!(c.value("p:fog"), Some(&ConfigValue::Number(0.0)));
        c.next_preset();
        assert_eq!(c.value("p:fog"), Some(&ConfigValue::Number(100.0)));
        c.step("p:fog", false);
        assert!(c.preset_label().ends_with("Custom"));
    }

    #[test]
    fn entries_without_a_translation_are_labelled_by_name() {
        let c = config();
        assert_eq!(c.label("p:fancy"), "fancy");
        assert_eq!(c.ids().len(), 3);
    }
}
