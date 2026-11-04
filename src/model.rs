use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Init {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub config: HostConfig,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostConfig {
    pub rules_file: String,
    pub assets_dir: String,
    pub asset_url: String,
    pub max_queue: u64,
    pub default_duration_ms: u64,
    pub gap_ms: u64,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            rules_file: "config/interactions.toml".into(),
            assets_dir: "overlay/assets".into(),
            asset_url: "./assets/".into(),
            max_queue: 30,
            default_duration_ms: 4000,
            gap_ms: 150,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InputEvent {
    pub platform: Platform,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
    pub user: InputUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputUser {
    pub id: String,
    pub name: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Twitch,
    Kick,
    Youtube,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Twitch => "twitch",
            Self::Kick => "kick",
            Self::Youtube => "youtube",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Broadcaster,
    Moderator,
    Vip,
    Subscriber,
    Everyone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerEvent {
    Redemption,
    Donation,
}

impl TriggerEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Redemption => "redemption",
            Self::Donation => "donation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rules {
    pub commands: Vec<CommandRule>,
    pub mod_actions: Vec<ModAction>,
    pub chat_combos: Vec<ChatCombo>,
    pub counters: Vec<CounterRule>,
    pub event_triggers: Vec<EventTrigger>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRule {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "everyone")]
    pub roles: Vec<Role>,
    #[serde(default = "platforms")]
    pub platforms: Vec<Platform>,
    #[serde(default)]
    pub global_cooldown_ms: u64,
    #[serde(default)]
    pub user_cooldown_ms: u64,
    #[serde(default)]
    pub steps: Vec<Effect>,
    #[serde(default)]
    pub emits: Vec<EmitAction>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModAction {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "platforms")]
    pub platforms: Vec<Platform>,
    #[serde(default)]
    pub global_cooldown_ms: u64,
    #[serde(default)]
    pub user_cooldown_ms: u64,
    #[serde(default)]
    pub steps: Vec<Effect>,
    #[serde(default)]
    pub emits: Vec<EmitAction>,
    pub combo: Option<ModCombo>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModCombo {
    pub threshold: u64,
    pub window_ms: u64,
    #[serde(flatten)]
    pub effect: Effect,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatCombo {
    pub name: String,
    #[serde(default = "platforms")]
    pub platforms: Vec<Platform>,
    pub threshold: u64,
    pub window_ms: u64,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default = "one")]
    pub min_token_length: u64,
    #[serde(default)]
    pub ignore_prefixes: Vec<String>,
    #[serde(flatten)]
    pub effect: Effect,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterRule {
    pub name: String,
    pub show_command: String,
    pub increment_command: String,
    pub decrement_command: Option<String>,
    pub initial: i64,
    #[serde(default = "i64_min")]
    pub min: i64,
    #[serde(default = "i64_max")]
    pub max: i64,
    #[serde(default = "mutation_roles")]
    pub mutation_roles: Vec<Role>,
    #[serde(flatten)]
    pub effect: Effect,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTrigger {
    pub name: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub event: TriggerEvent,
    #[serde(default = "platforms")]
    pub platforms: Vec<Platform>,
    pub reward_id: Option<String>,
    pub reward_title: Option<String>,
    pub currency: Option<String>,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    #[serde(default)]
    pub steps: Vec<Effect>,
    #[serde(default)]
    pub emits: Vec<EmitAction>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub text: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default = "default_variant")]
    pub variant: String,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmitAction {
    pub name: String,
    #[serde(default = "empty_payload")]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionSet<'a> {
    pub steps: &'a [Effect],
    pub emits: &'a [EmitAction],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionContext {
    pub source: String,
    pub rule: String,
    pub command: String,
    pub args: String,
    pub count: u64,
    pub platform: Platform,
    pub user: InputUser,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmissionContext {
    pub source: String,
    pub rule: String,
    pub command: String,
    pub args: String,
    pub count: u64,
    pub platform: Platform,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Presentation {
    pub schema: u8,
    pub source: String,
    pub rule: String,
    pub step: usize,
    pub text: Option<String>,
    pub image: Option<String>,
    pub sound: Option<String>,
    pub variant: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComboPresentation {
    pub schema: u8,
    pub kind: String,
    pub rule: String,
    pub key: String,
    pub count: u64,
    pub threshold: u64,
    pub text: Option<String>,
    pub image: Option<String>,
    pub sound: Option<String>,
    pub variant: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CounterPresentation {
    pub schema: u8,
    pub rule: String,
    pub name: String,
    pub value: i64,
    pub text: Option<String>,
    pub image: Option<String>,
    pub sound: Option<String>,
    pub variant: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FreeEmission {
    pub schema: u8,
    pub name: String,
    pub payload: Value,
    pub context: EmissionContext,
}

fn everyone() -> Vec<Role> {
    vec![Role::Everyone]
}

fn platforms() -> Vec<Platform> {
    vec![Platform::Twitch, Platform::Kick, Platform::Youtube]
}

fn one() -> u64 {
    1
}

fn i64_min() -> i64 {
    i64::MIN
}

fn i64_max() -> i64 {
    i64::MAX
}

fn mutation_roles() -> Vec<Role> {
    vec![Role::Moderator, Role::Broadcaster]
}

fn enabled() -> bool {
    true
}

fn default_variant() -> String {
    "default".into()
}

fn empty_payload() -> Value {
    Value::Object(Map::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_config_defaults_match_manifest() {
        let config: HostConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, HostConfig::default());
    }

    #[test]
    fn command_defaults_are_applied() {
        let rules: Rules = toml::from_str(
            r#"
            [[commands]]
            name = "hola"
            [[commands.steps]]
            text = "Hola"
            "#,
        )
        .unwrap();
        let command = &rules.commands[0];
        assert_eq!(command.roles, [Role::Everyone]);
        assert_eq!(
            command.platforms,
            [Platform::Twitch, Platform::Kick, Platform::Youtube]
        );
        assert_eq!(command.global_cooldown_ms, 0);
        assert_eq!(command.user_cooldown_ms, 0);
        assert!(command.aliases.is_empty());
        assert_eq!(command.steps[0].variant, "default");
    }

    #[test]
    fn complete_example_deserializes_every_rule() {
        let rules: Rules = toml::from_str(include_str!("../config/interactions.toml")).unwrap();
        assert_eq!(rules.commands.len(), 2);
        assert_eq!(rules.mod_actions.len(), 3);
        assert_eq!(rules.chat_combos.len(), 1);
        assert_eq!(rules.counters.len(), 1);
        assert_eq!(rules.event_triggers.len(), 2);
        assert_eq!(rules.commands[1].emits[0].name, "camera_move");
        assert_eq!(rules.mod_actions[0].combo.as_ref().unwrap().threshold, 3);
        assert_eq!(rules.counters[0].min, 0);
    }

    #[test]
    fn optional_rule_defaults_are_applied() {
        let rules: Rules = toml::from_str(
            r#"
            [[counters]]
            name = "n"
            show_command = "n"
            increment_command = "n+"
            initial = 0
            text = "{value}"

            [[event_triggers]]
            name = "donacion"
            event = "donation"
            [[event_triggers.steps]]
            text = "Gracias"
            "#,
        )
        .unwrap();
        assert_eq!(rules.counters[0].min, i64::MIN);
        assert_eq!(rules.counters[0].max, i64::MAX);
        assert_eq!(
            rules.counters[0].mutation_roles,
            [Role::Moderator, Role::Broadcaster]
        );
        assert!(rules.event_triggers[0].enabled);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<Rules>("unknown = true").is_err());
        assert!(toml::from_str::<Rules>("[[commands]]\nname = 'hola'\nunknown = true").is_err());
        assert!(serde_json::from_str::<HostConfig>(r#"{"unknown":true}"#).is_err());
    }
}
