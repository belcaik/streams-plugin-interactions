use std::{
    error::Error,
    fmt,
    io::{self, BufRead, Write},
};

use serde::Serialize;

use crate::model::{
    ComboPresentation, CounterPresentation, FreeEmission, Init, InputEvent, InputUser, Presentation,
};

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    Json(serde_json::Error),
    MissingInit,
    WrongInit,
    Limits(&'static str),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "error de E/S: {error}"),
            Self::Json(error) => write!(f, "JSON inválido: {error}"),
            Self::MissingInit => f.write_str("falta la línea init"),
            Self::WrongInit => f.write_str("la primera línea no es init"),
            Self::Limits(field) => write!(f, "límite inválido en {field}"),
        }
    }
}

impl Error for ProtocolError {}

impl From<io::Error> for ProtocolError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn read_init(reader: &mut impl BufRead) -> Result<Init, ProtocolError> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(ProtocolError::MissingInit);
    }
    let init: Init = serde_json::from_str(&line)?;
    if init.event_type != "init" {
        return Err(ProtocolError::WrongInit);
    }
    let config = &init.config;
    if !(1..=100).contains(&config.max_queue) {
        return Err(ProtocolError::Limits("max_queue"));
    }
    if !(100..=300_000).contains(&config.default_duration_ms) {
        return Err(ProtocolError::Limits("default_duration_ms"));
    }
    if config.gap_ms > 60_000 {
        return Err(ProtocolError::Limits("gap_ms"));
    }
    Ok(init)
}

pub fn parse_event(line: &str) -> Result<InputEvent, ProtocolError> {
    Ok(serde_json::from_str(line)?)
}

#[derive(Serialize)]
struct EventLine<'a, T: Serialize> {
    #[serde(rename = "type")]
    kind: &'static str,
    data: &'a T,
    user: &'a InputUser,
}

fn write_event<T: Serialize>(
    writer: &mut impl Write,
    kind: &'static str,
    data: &T,
    user: &InputUser,
) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &EventLine { kind, data, user })
        .map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

pub fn write_show(
    writer: &mut impl Write,
    value: &Presentation,
    user: &InputUser,
) -> io::Result<()> {
    write_event(writer, "interaction_show", value, user)
}

pub fn write_combo(
    writer: &mut impl Write,
    value: &ComboPresentation,
    user: &InputUser,
) -> io::Result<()> {
    write_event(writer, "interaction_combo", value, user)
}

pub fn write_counter(
    writer: &mut impl Write,
    value: &CounterPresentation,
    user: &InputUser,
) -> io::Result<()> {
    write_event(writer, "interaction_counter", value, user)
}

pub fn write_emit(
    writer: &mut impl Write,
    value: &FreeEmission,
    user: &InputUser,
) -> io::Result<()> {
    write_event(writer, "interaction_emit", value, user)
}
