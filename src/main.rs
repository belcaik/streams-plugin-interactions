pub mod config;
pub mod model;

use std::{error::Error, io, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct Init {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    #[allow(dead_code)]
    config: serde_json::Value,
    #[allow(dead_code)]
    data_dir: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    if io::BufRead::read_line(&mut input, &mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "falta la línea init").into());
    }
    let init: Init = serde_json::from_str(&line)?;
    if init.event_type != "init" {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "la primera línea no es init").into(),
        );
    }
    while io::BufRead::read_line(&mut input, &mut line)? != 0 {
        line.clear();
    }
    Ok(())
}
