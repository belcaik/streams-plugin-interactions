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
    fn new(root: &Path, asset_url: String) -> Result<Self, ConfigError> {
        let canonical = root.canonicalize().map_err(|error| {
            ConfigError::new(
                root,
                "assets_dir",
                format!("no se puede canonicalizar: {error}"),
            )
        })?;
        if !canonical.is_dir() {
            return Err(ConfigError::new(root, "assets_dir", "no es un directorio"));
        }
        Ok(Self {
            root: canonical,
            asset_url,
        })
    }

    pub fn public_url(
        &self,
        field: &str,
        value: Option<&str>,
    ) -> Result<Option<String>, ConfigError> {
        let Some(relative) = value else {
            return Ok(None);
        };
        if !valid_asset(relative) {
            return Err(ConfigError::new(
                &self.root,
                field,
                "ruta de asset inválida",
            ));
        }
        let candidate = self.root.join(relative);
        let canonical = candidate.canonicalize().map_err(|error| {
            ConfigError::new(&self.root, field, format!("asset inexistente: {error}"))
        })?;
        if !canonical.starts_with(&self.root) || !canonical.is_file() {
            return Err(ConfigError::new(
                &self.root,
                field,
                "asset fuera de la raíz o no regular",
            ));
        }
        Ok(Some(format!("{}{relative}", self.asset_url)))
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
    let assets = AssetRoot::new(
        &plugin_root.join(&init.config.assets_dir),
        init.config.asset_url.clone(),
    )?;
    preload_assets(&rules, &assets)?;
    Ok(Loaded {
        host: init.config.clone(),
        rules,
        assets,
    })
}

fn preload_assets(rules: &Rules, assets: &AssetRoot) -> Result<(), ConfigError> {
    let mut effects = Vec::new();
    for command in &rules.commands {
        effects.extend(&command.steps);
    }
    for action in &rules.mod_actions {
        effects.extend(&action.steps);
        effects.extend(action.combo.as_ref().map(|combo| &combo.effect));
    }
    effects.extend(rules.chat_combos.iter().map(|combo| &combo.effect));
    effects.extend(rules.counters.iter().map(|counter| &counter.effect));
    for trigger in &rules.event_triggers {
        effects.extend(&trigger.steps);
    }
    for effect in effects {
        assets.public_url("image", effect.image.as_deref())?;
        assets.public_url("sound", effect.sound.as_deref())?;
    }
    Ok(())
}

fn valid_asset(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
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
        for name in [&counter.show_command, &counter.increment_command] {
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn load_rules(toml: &str) -> Result<Loaded, ConfigError> {
        let root = std::env::temp_dir().join(format!(
            "interactions-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("overlay/assets")).unwrap();
        fs::write(root.join("config/interactions.toml"), toml).unwrap();
        let init = Init {
            event_type: "init".into(),
            config: HostConfig::default(),
            data_dir: root.join("data"),
        };
        load(&init, &root)
    }

    #[test]
    fn minimal_rule_loads() {
        let loaded =
            load_rules("[[commands]]\nname='hola'\n[[commands.steps]]\ntext='Hola'").unwrap();
        assert_eq!(loaded.rules.commands[0].name, "hola");
    }

    #[test]
    fn duplicate_alias_across_rule_kinds_fails() {
        let error = load_rules(
            r#"
            [[commands]]
            name = "hola"
            aliases = ["saludo"]
            [[commands.steps]]
            text = "Hola"
            [[mod_actions]]
            name = "saludo"
            [[mod_actions.steps]]
            text = "Hola"
            "#,
        )
        .err()
        .unwrap();
        assert!(error.reason().contains("duplicado"));
    }

    #[test]
    fn invalid_roles_and_empty_platforms_fail() {
        let everyone = load_rules(
            "[[commands]]\nname='x'\nroles=['everyone','vip']\n[[commands.steps]]\ntext='x'",
        );
        assert!(everyone.is_err());
        let invalid =
            load_rules("[[commands]]\nname='x'\nroles=['mod']\n[[commands.steps]]\ntext='x'");
        assert!(invalid.is_err());
        let empty =
            load_rules("[[commands]]\nname='x'\nplatforms=[]\n[[commands.steps]]\ntext='x'");
        assert!(empty.is_err());
    }

    #[test]
    fn placeholders_are_contextual_and_balanced() {
        for text in ["{unknown}", "{key}", "{user", "{user}}"] {
            let source = format!("[[commands]]\nname='x'\n[[commands.steps]]\ntext='{text}'");
            assert!(load_rules(&source).is_err(), "accepted {text}");
        }
        let combo = load_rules(
            "[[chat_combos]]\nname='x'\nthreshold=2\nwindow_ms=1000\ntext='{key} {count}'",
        );
        assert!(combo.is_ok());
        let counter = load_rules(
            "[[counters]]\nname='n'\nshow_command='n'\nincrement_command='n+'\ninitial=0\ntext='{name}: {value}'",
        );
        assert!(counter.is_ok());
    }

    #[test]
    fn rule_limits_are_enforced() {
        for source in [
            "[[chat_combos]]\nname='x'\nthreshold=1\nwindow_ms=1000\ntext='x'",
            "[[chat_combos]]\nname='x'\nthreshold=2\nwindow_ms=99\ntext='x'",
            "[[chat_combos]]\nname='x'\nthreshold=2\nwindow_ms=1000\nmin_token_length=65\ntext='x'",
            "[[commands]]\nname='x'\n[[commands.steps]]\ntext='x'\nduration_ms=99",
            "[[counters]]\nname='n'\nshow_command='n'\nincrement_command='n+'\ninitial=11\nmin=0\nmax=10\ntext='x'",
        ] {
            assert!(load_rules(source).is_err(), "accepted {source}");
        }
    }

    #[test]
    fn triggers_require_filters_and_valid_amounts() {
        let redemption = load_rules(
            "[[event_triggers]]\nname='r'\nevent='redemption'\n[[event_triggers.steps]]\ntext='x'",
        );
        assert!(redemption.is_err());
        let donation = load_rules(
            "[[event_triggers]]\nname='d'\nevent='donation'\nmin_amount=10.0\nmax_amount=1.0\n[[event_triggers.steps]]\ntext='x'",
        );
        assert!(donation.is_err());
    }

    #[test]
    fn free_event_names_and_variants_are_restricted() {
        let emit = load_rules("[[commands]]\nname='x'\n[[commands.emits]]\nname='Bad Event'");
        assert!(emit.is_err());
        let variant = load_rules(
            "[[commands]]\nname='x'\n[[commands.steps]]\ntext='x'\nvariant='bad_variant'",
        );
        assert!(variant.is_err());
    }
}
