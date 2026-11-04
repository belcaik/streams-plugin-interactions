use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::model::CounterRule;

const VERSION: u64 = 1;

#[derive(Debug)]
pub struct PersistenceError {
    path: PathBuf,
    reason: String,
}

impl PersistenceError {
    fn new(path: &Path, reason: impl fmt::Display) -> Self {
        Self {
            path: path.into(),
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.reason)
    }
}

impl Error for PersistenceError {}

#[derive(Deserialize)]
struct FileVersion {
    version: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct CounterFile {
    version: u64,
    values: BTreeMap<String, i64>,
}

#[derive(Debug)]
pub struct CounterStore {
    path: PathBuf,
    values: BTreeMap<String, i64>,
}

impl CounterStore {
    pub fn load(data_dir: &Path, rules: &[CounterRule]) -> Result<Self, PersistenceError> {
        fs::create_dir_all(data_dir).map_err(|error| {
            PersistenceError::new(data_dir, format!("no se puede crear data_dir: {error}"))
        })?;
        let path = data_dir.join("counters.json");
        let mut values = match fs::read_to_string(&path) {
            Ok(text) => parse(&path, &text)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => {
                return Err(PersistenceError::new(
                    &path,
                    format!("no se puede leer: {error}"),
                ));
            }
        };
        for rule in rules {
            values.entry(rule.name.clone()).or_insert(rule.initial);
        }
        Ok(Self { path, values })
    }

    pub fn get(&self, name: &str) -> Option<i64> {
        self.values.get(name).copied()
    }

    pub fn set_and_save(&mut self, name: &str, value: i64) -> Result<(), PersistenceError> {
        let mut values = self.values.clone();
        values.insert(name.into(), value);
        save(&self.path, values.clone()).map_err(|error| {
            PersistenceError::new(&self.path, format!("no se pudo guardar: {error}"))
        })?;
        self.values = values;
        Ok(())
    }
}

fn parse(path: &Path, text: &str) -> Result<BTreeMap<String, i64>, PersistenceError> {
    let invalid = |error| PersistenceError::new(path, format!("JSON inválido: {error}"));
    let FileVersion { version } = serde_json::from_str(text).map_err(invalid)?;
    if version != VERSION {
        return Err(PersistenceError::new(
            path,
            format!("versión {version} no compatible; se esperaba {VERSION}"),
        ));
    }
    let file: CounterFile = serde_json::from_str(text).map_err(invalid)?;
    Ok(file.values)
}

fn save(path: &Path, values: BTreeMap<String, i64>) -> io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    let result = write_temporary(&temporary, values).and_then(|()| replace(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_temporary(temporary: &Path, values: BTreeMap<String, i64>) -> io::Result<()> {
    let mut file = File::create(temporary)?;
    let bytes = serde_json::to_vec(&CounterFile {
        version: VERSION,
        values,
    })
    .map_err(io::Error::other)?;
    file.write_all(&bytes)?;
    file.flush()?;
    file.sync_all()
}

#[cfg(not(windows))]
fn replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    let backup = destination.with_extension("json.bak");
    let had_destination = destination.exists();
    if had_destination {
        let _ = fs::remove_file(&backup);
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(temporary, destination) {
        if had_destination {
            let _ = fs::rename(&backup, destination);
        }
        return Err(error);
    }
    if had_destination {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::model::{Effect, Role};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "interactions-persistence-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn rule(name: &str, initial: i64) -> CounterRule {
        CounterRule {
            name: name.into(),
            show_command: name.into(),
            increment_command: format!("{name}+"),
            decrement_command: None,
            initial,
            min: i64::MIN,
            max: i64::MAX,
            mutation_roles: vec![Role::Moderator],
            effect: Effect {
                text: Some("{value}".into()),
                image: None,
                sound: None,
                variant: "counter".into(),
                duration_ms: None,
            },
        }
    }

    #[test]
    fn missing_file_uses_initial_values_and_creates_data_dir() {
        let directory = directory().join("nested");
        let store = CounterStore::load(&directory, &[rule("muertes", 3)]).unwrap();
        assert_eq!(store.get("muertes"), Some(3));
        assert!(directory.is_dir());
        assert!(!directory.join("counters.json").exists());
    }

    #[test]
    fn saved_value_survives_restart_with_stable_bytes() {
        let directory = directory();
        let rules = [rule("muertes", 0)];
        let mut store = CounterStore::load(&directory, &rules).unwrap();
        store.set_and_save("muertes", 7).unwrap();
        assert_eq!(
            fs::read_to_string(directory.join("counters.json")).unwrap(),
            r#"{"version":1,"values":{"muertes":7}}"#
        );
        let restarted = CounterStore::load(&directory, &rules).unwrap();
        assert_eq!(restarted.get("muertes"), Some(7));
    }
}
