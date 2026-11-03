use std::{
    error::Error,
    fmt,
    io::{self, Write},
};

use crate::{
    clock::Clock,
    config::{AssetRoot, ConfigError, Loaded},
    model::{
        ActionSet, CommandRule, Effect, EmissionContext, ExecutionContext, FreeEmission,
        InputEvent, InputUser, Platform, Presentation,
    },
    protocol::{write_emit, write_show},
};

#[derive(Debug)]
pub enum EngineError {
    Config(ConfigError),
    Io(io::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl Error for EngineError {}

impl From<ConfigError> for EngineError {
    fn from(value: ConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<io::Error> for EngineError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

struct ActionRequest<'a> {
    source: &'a str,
    rule: &'a str,
    command: &'a str,
    args: &'a str,
    actions: ActionSet<'a>,
}

pub struct Engine<C: Clock> {
    loaded: Loaded,
    clock: C,
    executions: std::collections::HashMap<String, u64>,
    global_cooldowns: std::collections::HashMap<String, u64>,
    user_cooldowns: std::collections::HashMap<(String, Platform, String), u64>,
    mod_trackers: std::collections::HashMap<(Platform, String), Vec<(String, u64)>>,
}

impl<C: Clock> Engine<C> {
    pub fn new(loaded: Loaded, clock: C) -> Self {
        Self {
            loaded,
            clock,
            executions: std::collections::HashMap::new(),
            global_cooldowns: std::collections::HashMap::new(),
            user_cooldowns: std::collections::HashMap::new(),
            mod_trackers: std::collections::HashMap::new(),
        }
    }

    pub fn handle(&mut self, event: InputEvent, out: &mut impl Write) -> Result<(), EngineError> {
        if event.kind != "chat" {
            return Ok(());
        }
        let Some(message) = event.data.get("message").and_then(|value| value.as_str()) else {
            return Ok(());
        };
        let Some((command, args)) = parse_command(message) else {
            return Ok(());
        };
        if let Some(rule) = self.find_command(&command) {
            if !platform_allowed(&rule.platforms, event.platform)
                || !role_allowed(&rule.roles, &event.user)
                || !self.cooldown_allowed(
                    &rule.name,
                    rule.global_cooldown_ms,
                    rule.user_cooldown_ms,
                    &event,
                )
            {
                return Ok(());
            }
            self.run_actions(
                ActionRequest {
                    source: "command",
                    rule: &rule.name,
                    command: &command,
                    args,
                    actions: ActionSet {
                        steps: &rule.steps,
                        emits: &rule.emits,
                    },
                },
                &event,
                out,
            )?;
            self.record_cooldown(&rule.name, event.platform, event.user.id.clone());
            return Ok(());
        }
        if let Some(rule) = self.find_mod(&command) {
            let roles = [
                crate::model::Role::Moderator,
                crate::model::Role::Broadcaster,
            ];
            if !platform_allowed(&rule.platforms, event.platform)
                || !role_allowed(&roles, &event.user)
                || !self.cooldown_allowed(
                    &rule.name,
                    rule.global_cooldown_ms,
                    rule.user_cooldown_ms,
                    &event,
                )
            {
                return Ok(());
            }
            self.run_actions(
                ActionRequest {
                    source: "mod_action",
                    rule: &rule.name,
                    command: &command,
                    args,
                    actions: ActionSet {
                        steps: &rule.steps,
                        emits: &rule.emits,
                    },
                },
                &event,
                out,
            )?;
            self.record_cooldown(&rule.name, event.platform, event.user.id.clone());
            self.track_mod(&rule, &command, args, &event, out)?;
        }
        Ok(())
    }

    fn cooldown_allowed(
        &mut self,
        name: &str,
        global_ms: u64,
        user_ms: u64,
        event: &InputEvent,
    ) -> bool {
        let now = self.clock.now_ms();
        self.global_cooldowns
            .retain(|key, at| key != name || now.saturating_sub(*at) < global_ms);
        self.user_cooldowns
            .retain(|(key, _, _), at| key != name || now.saturating_sub(*at) < user_ms);
        let global = self
            .global_cooldowns
            .get(name)
            .is_some_and(|at| global_ms > 0 && now.saturating_sub(*at) < global_ms);
        let user = self
            .user_cooldowns
            .get(&(name.into(), event.platform, event.user.id.clone()))
            .is_some_and(|at| user_ms > 0 && now.saturating_sub(*at) < user_ms);
        !global && !user
    }

    fn record_cooldown(&mut self, name: &str, platform: Platform, user_id: String) {
        let now = self.clock.now_ms();
        self.global_cooldowns.insert(name.into(), now);
        self.user_cooldowns
            .insert((name.into(), platform, user_id), now);
    }

    fn find_command(&self, command: &str) -> Option<CommandRule> {
        self.loaded
            .rules
            .commands
            .iter()
            .find(|rule| rule.name == command || rule.aliases.iter().any(|alias| alias == command))
            .cloned()
    }

    fn find_mod(&self, command: &str) -> Option<crate::model::ModAction> {
        self.loaded
            .rules
            .mod_actions
            .iter()
            .find(|rule| rule.name == command || rule.aliases.iter().any(|alias| alias == command))
            .cloned()
    }

    fn run_actions(
        &mut self,
        request: ActionRequest<'_>,
        event: &InputEvent,
        out: &mut impl Write,
    ) -> Result<(), EngineError> {
        let count = self.next_count(request.rule);
        self.execute(
            request.actions,
            ExecutionContext {
                source: request.source.into(),
                rule: request.rule.into(),
                command: request.command.into(),
                args: request.args.into(),
                count,
                platform: event.platform,
                user: event.user.clone(),
            },
            out,
        )
    }

    fn track_mod(
        &mut self,
        rule: &crate::model::ModAction,
        command: &str,
        args: &str,
        event: &InputEvent,
        out: &mut impl Write,
    ) -> Result<(), EngineError> {
        let Some(combo) = &rule.combo else {
            return Ok(());
        };
        let now = self.clock.now_ms();
        let tracker = self
            .mod_trackers
            .entry((event.platform, rule.name.clone()))
            .or_default();
        prune_mods(tracker, now, combo.window_ms);
        if !tracker.iter().any(|(id, _)| id == &event.user.id) {
            tracker.push((event.user.id.clone(), now));
        }
        if tracker.len() < combo.threshold as usize {
            return Ok(());
        }
        let count = tracker.len() as u64;
        let context = ExecutionContext {
            source: "mod_action".into(),
            rule: rule.name.clone(),
            command: command.into(),
            args: args.into(),
            count,
            platform: event.platform,
            user: event.user.clone(),
        };
        let presentation = crate::model::ComboPresentation {
            schema: 1,
            kind: "moderators".into(),
            rule: rule.name.clone(),
            key: rule.name.clone(),
            count,
            threshold: combo.threshold,
            text: combo
                .effect
                .text
                .as_ref()
                .map(|text| render(text, &context, None)),
            image: self
                .loaded
                .assets
                .public_url("image", combo.effect.image.as_deref())?,
            sound: self
                .loaded
                .assets
                .public_url("sound", combo.effect.sound.as_deref())?,
            variant: combo.effect.variant.clone(),
            duration_ms: combo
                .effect
                .duration_ms
                .unwrap_or(self.loaded.host.default_duration_ms),
        };
        crate::protocol::write_combo(out, &presentation, &event.user)?;
        tracker.clear();
        Ok(())
    }

    fn next_count(&mut self, rule: &str) -> u64 {
        let count = self.executions.entry(rule.into()).or_default();
        *count = count.saturating_add(1);
        *count
    }

    fn execute(
        &mut self,
        actions: ActionSet<'_>,
        context: ExecutionContext,
        out: &mut impl Write,
    ) -> Result<(), EngineError> {
        for (step, effect) in actions.steps.iter().enumerate() {
            let presentation = effect_presentation(
                &self.loaded.assets,
                &context,
                effect,
                step,
                self.loaded.host.default_duration_ms,
            )?;
            write_show(out, &presentation, &context.user)?;
        }
        emit_actions(actions.emits, &context, out)?;
        Ok(())
    }
}

pub fn prune_mods(entries: &mut Vec<(String, u64)>, now: u64, window_ms: u64) {
    entries.retain(|(_, at)| now.saturating_sub(*at) < window_ms);
}

pub fn parse_command(message: &str) -> Option<(String, &str)> {
    let rest = message.strip_prefix('!')?;
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let name = rest[..end].to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    let args = rest[end..].trim();
    Some((name, args))
}

pub fn first_arg(args: &str) -> &str {
    args.split_whitespace().next().unwrap_or("")
}

pub fn render(template: &str, context: &ExecutionContext, key: Option<&str>) -> String {
    let mut text = template
        .replace("{user}", &context.user.name)
        .replace("{args}", &context.args)
        .replace("{target}", first_arg(&context.args))
        .replace("{count}", &context.count.to_string())
        .replace("{platform}", context.platform.as_str())
        .replace("{command}", &context.command);
    if let Some(key) = key {
        text = text.replace("{key}", key);
    }
    text
}

pub fn effect_presentation(
    assets: &AssetRoot,
    context: &ExecutionContext,
    effect: &Effect,
    step: usize,
    default_duration_ms: u64,
) -> Result<Presentation, ConfigError> {
    Ok(Presentation {
        schema: 1,
        source: context.source.clone(),
        rule: context.rule.clone(),
        step,
        text: effect.text.as_ref().map(|text| render(text, context, None)),
        image: assets.public_url("image", effect.image.as_deref())?,
        sound: assets.public_url("sound", effect.sound.as_deref())?,
        variant: effect.variant.clone(),
        duration_ms: effect.duration_ms.unwrap_or(default_duration_ms),
    })
}

pub fn emit_actions(
    actions: &[crate::model::EmitAction],
    context: &ExecutionContext,
    out: &mut impl Write,
) -> io::Result<()> {
    for action in actions {
        let emission = FreeEmission {
            schema: 1,
            name: action.name.clone(),
            payload: action.payload.clone(),
            context: EmissionContext {
                source: context.source.clone(),
                rule: context.rule.clone(),
                command: context.command.clone(),
                args: context.args.clone(),
                count: context.count,
                platform: context.platform,
            },
        };
        write_emit(out, &emission, &context.user)?;
    }
    Ok(())
}

pub fn platform_allowed(platforms: &[Platform], platform: Platform) -> bool {
    platforms.contains(&platform)
}

pub fn role_allowed(roles: &[crate::model::Role], user: &InputUser) -> bool {
    roles.iter().any(|role| {
        let name = format!("{role:?}").to_ascii_lowercase();
        user.roles.iter().any(|actual| actual == &name)
            || *role == crate::model::Role::Everyone
            || *role == crate::model::Role::Moderator
                && user.roles.iter().any(|actual| actual == "broadcaster")
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde_json::Value;

    use super::*;
    use crate::{
        clock::ManualClock,
        model::{HostConfig, Init},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn engine(rules: &str) -> Engine<ManualClock> {
        let root = std::env::temp_dir().join(format!(
            "interactions-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("overlay/assets")).unwrap();
        fs::write(root.join("config/interactions.toml"), rules).unwrap();
        let init = Init {
            event_type: "init".into(),
            config: HostConfig::default(),
            data_dir: root.join("data"),
        };
        Engine::new(
            crate::config::load(&init, &root).unwrap(),
            ManualClock::new(0),
        )
    }

    fn chat(message: &str, roles: &[&str]) -> InputEvent {
        InputEvent {
            platform: Platform::Twitch,
            kind: "chat".into(),
            data: serde_json::json!({"message": message}),
            user: InputUser {
                id: "u1".into(),
                name: "Ana".into(),
                roles: roles.iter().map(|role| (*role).into()).collect(),
            },
        }
    }

    fn output(engine: &mut Engine<ManualClock>, event: InputEvent) -> Vec<Value> {
        let mut out = Vec::new();
        engine.handle(event, &mut out).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn command_executes_steps_and_free_emissions_in_order() {
        let mut engine = engine(
            "[[commands]]\nname='hola'\naliases=['hi']\n[[commands.steps]]\ntext='Hola {user}: {args}'\n[[commands.steps]]\ntext='Otra vez {count}'\n[[commands.emits]]\nname='camera_move'\npayload={preset='desk'}",
        );
        let events = output(&mut engine, chat("!HI uno dos", &[]));
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["data"]["text"], "Hola Ana: uno dos");
        assert_eq!(events[1]["data"]["step"], 1);
        assert_eq!(events[2]["data"]["name"], "camera_move");
    }

    #[test]
    fn roles_and_platforms_are_checked_before_execution() {
        let mut engine = engine(
            "[[commands]]\nname='hype'\nroles=['moderator']\nplatforms=['kick']\n[[commands.steps]]\ntext='Hype'",
        );
        assert!(output(&mut engine, chat("!hype", &[])).is_empty());
        assert!(output(&mut engine, chat("!hype", &["broadcaster"])).is_empty());
        let mut allowed = chat("!hype", &["broadcaster"]);
        allowed.platform = Platform::Kick;
        assert_eq!(output(&mut engine, allowed).len(), 1);
    }

    #[test]
    fn global_cooldown_uses_the_injected_clock() {
        let mut engine = engine(
            "[[commands]]\nname='x'\nglobal_cooldown_ms=1000\n[[commands.steps]]\ntext='{count}'",
        );
        assert_eq!(output(&mut engine, chat("!x", &[])).len(), 1);
        assert!(output(&mut engine, chat("!x", &[])).is_empty());
        engine.clock.advance(1000);
        assert_eq!(output(&mut engine, chat("!x", &[]))[0]["data"]["text"], "2");
    }

    #[test]
    fn user_cooldown_is_isolated_by_user_and_platform() {
        let mut engine = engine(
            "[[commands]]\nname='x'\nuser_cooldown_ms=1000\n[[commands.steps]]\ntext='{count}'",
        );
        assert_eq!(output(&mut engine, chat("!x", &[])).len(), 1);
        assert!(output(&mut engine, chat("!x", &[])).is_empty());
        let mut other = chat("!x", &[]);
        other.user.id = "u2".into();
        assert_eq!(output(&mut engine, other).len(), 1);
        let mut kick = chat("!x", &[]);
        kick.platform = Platform::Kick;
        assert_eq!(output(&mut engine, kick).len(), 1);
    }

    #[test]
    fn command_parser_is_anchored_and_preserves_unicode_arguments() {
        assert!(parse_command(" !hola").is_none());
        assert_eq!(
            parse_command("!MuErTeS+  Ána  dos  ").unwrap(),
            ("muertes+".into(), "Ána  dos")
        );
        assert!(parse_command("!").is_none());
    }

    #[test]
    fn canonical_roles_include_everyone_vip_and_subscriber() {
        let vip = chat("x", &["vip"]);
        let subscriber = chat("x", &["subscriber"]);
        assert!(role_allowed(&[crate::model::Role::Everyone], &vip.user));
        assert!(role_allowed(&[crate::model::Role::Vip], &vip.user));
        assert!(role_allowed(
            &[crate::model::Role::Subscriber],
            &subscriber.user
        ));
    }
}
