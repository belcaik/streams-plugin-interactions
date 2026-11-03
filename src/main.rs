pub mod clock;
pub mod config;
pub mod engine;
pub mod model;
pub mod protocol;

use std::{
    error::Error,
    io::{self, BufRead, Write},
    path::Path,
};

use crate::{
    clock::{Clock, SystemClock},
    engine::{Engine, EngineError},
    protocol::{parse_event, read_init},
};

fn run(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    plugin_root: &Path,
    clock: impl Clock,
) -> Result<(), Box<dyn Error>> {
    let init = read_init(reader)?;
    let loaded = config::load(&init, plugin_root)?;
    let mut engine = Engine::new(loaded, clock);
    for line in reader.lines() {
        let line = line?;
        let event = match parse_event(&line) {
            Ok(event) => event,
            Err(error) => {
                eprintln!("{error}");
                continue;
            }
        };
        match engine.handle(event, writer) {
            Ok(()) | Err(EngineError::Config(_)) => {}
            Err(EngineError::Io(error)) => return Err(error.into()),
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = std::env::current_dir()?;
    run(
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        &root,
        SystemClock::default(),
    )
}
