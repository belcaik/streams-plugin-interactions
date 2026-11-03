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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::json;

    use super::*;
    use crate::model::{EmissionContext, Platform};

    fn user() -> InputUser {
        InputUser {
            id: "u1".into(),
            name: "Ana".into(),
            roles: vec![],
        }
    }

    #[test]
    fn init_defaults_and_host_limits_are_checked() {
        let init = read_init(&mut Cursor::new(
            br#"{"type":"init","config":{},"data_dir":"/tmp/data"}"#,
        ))
        .unwrap();
        assert_eq!(init.config.max_queue, 30);
        for config in [
            r#"{"max_queue":0}"#,
            r#"{"max_queue":101}"#,
            r#"{"default_duration_ms":99}"#,
            r#"{"gap_ms":60001}"#,
        ] {
            let line = format!(r#"{{"type":"init","config":{config},"data_dir":"."}}"#);
            assert!(read_init(&mut Cursor::new(line)).is_err());
        }
        assert!(read_init(&mut Cursor::new(b"")).is_err());
        assert!(read_init(&mut Cursor::new(br#"{"type":"chat"}"#)).is_err());
    }

    #[test]
    fn input_event_accepts_envelope_metadata() {
        let event = parse_event(
            r#"{"id":"e1","ts":"now","platform":"twitch","type":"chat","data":{"message":"!hola"},"user":{"id":"u1","name":"Ana","roles":[]}}"#,
        )
        .unwrap();
        assert_eq!(event.platform, Platform::Twitch);
        assert_eq!(event.kind, "chat");
    }

    #[test]
    fn show_writer_matches_contract_byte_for_byte() {
        let value = Presentation {
            schema: 1,
            source: "command".into(),
            rule: "hello".into(),
            step: 0,
            text: Some("Hola Ana".into()),
            image: None,
            sound: None,
            variant: "default".into(),
            duration_ms: 4000,
        };
        let mut out = Vec::new();
        write_show(&mut out, &value, &user()).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "{\"type\":\"interaction_show\",\"data\":{\"schema\":1,\"source\":\"command\",\"rule\":\"hello\",\"step\":0,\"text\":\"Hola Ana\",\"image\":null,\"sound\":null,\"variant\":\"default\",\"duration_ms\":4000},\"user\":{\"id\":\"u1\",\"name\":\"Ana\",\"roles\":[]}}\n"
        );
    }

    #[test]
    fn remaining_writers_use_exact_event_types_and_payloads() {
        let combo = ComboPresentation {
            schema: 1,
            kind: "chat".into(),
            rule: "r".into(),
            key: "POG".into(),
            count: 5,
            threshold: 5,
            text: Some("x5 POG".into()),
            image: None,
            sound: None,
            variant: "combo".into(),
            duration_ms: 2500,
        };
        let counter = CounterPresentation {
            schema: 1,
            rule: "n".into(),
            name: "n".into(),
            value: 7,
            text: Some("N: 7".into()),
            image: None,
            sound: None,
            variant: "counter".into(),
            duration_ms: 3500,
        };
        let emit = FreeEmission {
            schema: 1,
            name: "camera_move".into(),
            payload: json!({"preset":"desk"}),
            context: EmissionContext {
                source: "command".into(),
                rule: "camara".into(),
                command: "camara".into(),
                args: "desk".into(),
                count: 1,
                platform: Platform::Twitch,
            },
        };
        let mut out = Vec::new();
        write_combo(&mut out, &combo, &user()).unwrap();
        write_counter(&mut out, &counter, &user()).unwrap();
        write_emit(&mut out, &emit, &user()).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("{\"type\":\"interaction_combo\""));
        assert!(lines[1].starts_with("{\"type\":\"interaction_counter\""));
        assert_eq!(
            lines[2],
            "{\"type\":\"interaction_emit\",\"data\":{\"schema\":1,\"name\":\"camera_move\",\"payload\":{\"preset\":\"desk\"},\"context\":{\"source\":\"command\",\"rule\":\"camara\",\"command\":\"camara\",\"args\":\"desk\",\"count\":1,\"platform\":\"twitch\"}},\"user\":{\"id\":\"u1\",\"name\":\"Ana\",\"roles\":[]}}"
        );
    }
}
