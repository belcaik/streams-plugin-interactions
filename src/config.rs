use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use crate::model::{ActionSet, Effect, EventTrigger, HostConfig, Init, Role, Rules, TriggerEvent};

#[derive(Debug)]
pub struct ConfigError {
    path: PathBuf,
    field: String,
    reason: String,
}

impl ConfigError {
    fn new(path: &Path, field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            field: field.into(),
            reason: reason.into(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: campo {}: {}",
            self.path.display(),
            self.field,
            self.reason
        )
    }
}

impl Error for ConfigError {}

pub struct Loaded {
    pub host: HostConfig,
    pub rules: Rules,
    pub assets: AssetRoot,
}

pub struct AssetRoot {
    root: PathBuf,
    asset_url: String,
}

impl AssetRoot {
    pub fn public_url(
        &self,
        field: &str,
        value: Option<&str>,
    ) -> Result<Option<String>, ConfigError> {
        let _ = (&self.root, field);
        Ok(value.map(|path| format!("{}{path}", self.asset_url)))
    }
}

pub fn load(init: &Init, plugin_root: &Path) -> Result<Loaded, ConfigError> {
    let path = plugin_root.join(&init.config.rules_file);
    let text = fs::read_to_string(&path)
        .map_err(|error| ConfigError::new(&path, "rules_file", error.to_string()))?;
    let rules: Rules = toml::from_str(&text)
        .map_err(|error| ConfigError::new(&path, "rules", error.to_string()))?;
    validate(&rules, &path)?;
    if !init.config.asset_url.starts_with("./assets/") || !init.config.asset_url.ends_with('/') {
        return Err(ConfigError::new(
            &path,
            "asset_url",
            "debe usar ./assets/ y terminar en /",
        ));
    }
    Ok(Loaded {
        assets: AssetRoot {
            root: plugin_root.join(&init.config.assets_dir),
            asset_url: init.config.asset_url.clone(),
        },
        host: init.config.clone(),
        rules,
    })
}

fn validate(rules: &Rules, path: &Path) -> Result<(), ConfigError> {
    let mut names = HashSet::new();
    for command in &rules.commands {
        register(path, &mut names, &command.name)?;
        register_all(path, &mut names, &command.aliases)?;
        roles(path, &command.roles)?;
        platforms(path, command.platforms.len())?;
        actions(
            path,
            &command.name,
            ActionSet {
                steps: &command.steps,
                emits: &command.emits,
            },
            &[],
        )?;
    }
    for action in &rules.mod_actions {
        register(path, &mut names, &action.name)?;
        register_all(path, &mut names, &action.aliases)?;
        platforms(path, action.platforms.len())?;
        actions(
            path,
            &action.name,
            ActionSet {
                steps: &action.steps,
                emits: &action.emits,
            },
            &[],
        )?;
        if let Some(combo) = &action.combo {
            bounds(path, "combo.threshold", combo.threshold, 2, 100)?;
            bounds(path, "combo.window_ms", combo.window_ms, 100, 300_000)?;
            effect(path, &action.name, &combo.effect, &[])?;
        }
    }
    for combo in &rules.chat_combos {
        bounds(path, "threshold", combo.threshold, 2, 100)?;
        bounds(path, "window_ms", combo.window_ms, 100, 300_000)?;
        bounds(path, "min_token_length", combo.min_token_length, 1, 64)?;
        platforms(path, combo.platforms.len())?;
        effect(path, &combo.name, &combo.effect, &["key"])?;
    }
    for counter in &rules.counters {
        for name in [
            &counter.name,
            &counter.show_command,
            &counter.increment_command,
        ] {
            register(path, &mut names, name)?;
        }
        if let Some(name) = &counter.decrement_command {
            register(path, &mut names, name)?;
        }
        roles(path, &counter.mutation_roles)?;
        if counter.min > counter.max || !(counter.min..=counter.max).contains(&counter.initial) {
            return Err(ConfigError::new(
                path,
                &counter.name,
                "initial debe estar entre min y max",
            ));
        }
        effect(path, &counter.name, &counter.effect, &["name", "value"])?;
    }
    for event_trigger in &rules.event_triggers {
        trigger(path, event_trigger)?;
    }
    Ok(())
}

fn register_all(
    path: &Path,
    names: &mut HashSet<String>,
    values: &[String],
) -> Result<(), ConfigError> {
    for value in values {
        register(path, names, value)?;
    }
    Ok(())
}

fn register(path: &Path, names: &mut HashSet<String>, name: &str) -> Result<(), ConfigError> {
    if !valid_name(name, true) {
        return Err(ConfigError::new(path, name, "nombre de comando inválido"));
    }
    if !names.insert(name.to_ascii_lowercase()) {
        return Err(ConfigError::new(path, name, "nombre o alias duplicado"));
    }
    Ok(())
}

fn roles(path: &Path, roles: &[Role]) -> Result<(), ConfigError> {
    if roles.len() > 1 && roles.contains(&Role::Everyone) {
        return Err(ConfigError::new(
            path,
            "roles",
            "everyone debe aparecer solo",
        ));
    }
    Ok(())
}

fn platforms(path: &Path, count: usize) -> Result<(), ConfigError> {
    if count == 0 {
        return Err(ConfigError::new(
            path,
            "platforms",
            "la lista no puede estar vacía",
        ));
    }
    Ok(())
}

fn actions(
    path: &Path,
    rule: &str,
    actions: ActionSet<'_>,
    extra: &[&str],
) -> Result<(), ConfigError> {
    if actions.steps.is_empty() && actions.emits.is_empty() {
        return Err(ConfigError::new(
            path,
            rule,
            "debe tener un paso o una emisión",
        ));
    }
    for effect_value in actions.steps {
        effect(path, rule, effect_value, extra)?;
    }
    for emit in actions.emits {
        if !valid_name(&emit.name, false) {
            return Err(ConfigError::new(path, rule, "nombre de evento inválido"));
        }
    }
    Ok(())
}

fn effect(path: &Path, rule: &str, value: &Effect, extra: &[&str]) -> Result<(), ConfigError> {
    if !valid_name(&value.variant, false) || value.variant.contains('_') {
        return Err(ConfigError::new(path, rule, "variant inválido"));
    }
    if let Some(duration) = value.duration_ms {
        bounds(path, "duration_ms", duration, 100, 300_000)?;
    }
    if let Some(text) = &value.text {
        placeholders(path, rule, text, extra)?;
    }
    Ok(())
}

fn placeholders(path: &Path, rule: &str, text: &str, extra: &[&str]) -> Result<(), ConfigError> {
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            return Err(ConfigError::new(path, rule, "llaves sin balancear"));
        };
        let name = &rest[start + 1..start + end];
        let common = ["user", "args", "target", "count", "platform", "command"];
        if !common.contains(&name) && !extra.contains(&name) {
            return Err(ConfigError::new(
                path,
                rule,
                format!("placeholder desconocido {name}"),
            ));
        }
        rest = &rest[start + end + 1..];
    }
    if rest.contains('}') || text.matches('}').count() != text.matches('{').count() {
        return Err(ConfigError::new(path, rule, "llaves sin balancear"));
    }
    Ok(())
}

fn trigger(path: &Path, value: &EventTrigger) -> Result<(), ConfigError> {
    platforms(path, value.platforms.len())?;
    actions(
        path,
        &value.name,
        ActionSet {
            steps: &value.steps,
            emits: &value.emits,
        },
        &[],
    )?;
    match value.event {
        TriggerEvent::Redemption if value.reward_id.is_none() && value.reward_title.is_none() => {
            Err(ConfigError::new(
                path,
                &value.name,
                "falta reward_id o reward_title",
            ))
        }
        TriggerEvent::Donation => {
            if let Some(min) = value.min_amount {
                finite(path, &value.name, min)?;
            }
            if let Some(max) = value.max_amount {
                finite(path, &value.name, max)?;
            }
            if value
                .min_amount
                .zip(value.max_amount)
                .is_some_and(|(a, b)| a > b)
            {
                return Err(ConfigError::new(
                    path,
                    &value.name,
                    "min_amount supera max_amount",
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn finite(path: &Path, field: &str, value: f64) -> Result<(), ConfigError> {
    if !value.is_finite() || value < 0.0 {
        return Err(ConfigError::new(
            path,
            field,
            "importe no finito o negativo",
        ));
    }
    Ok(())
}

fn bounds(path: &Path, field: &str, value: u64, min: u64, max: u64) -> Result<(), ConfigError> {
    if !(min..=max).contains(&value) {
        return Err(ConfigError::new(
            path,
            field,
            format!("fuera de {min}..={max}"),
        ));
    }
    Ok(())
}

fn valid_name(value: &str, plus: bool) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' || plus && c == '+'
        })
}
