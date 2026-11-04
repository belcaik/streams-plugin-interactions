use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

const RULES: &str = r#"
[[commands]]
name = "hola"
roles = ["subscriber"]

[[commands.steps]]
text = "Hola {user}: {args}"
duration_ms = 3000

[[mod_actions]]
name = "hype"

[[mod_actions.steps]]
text = "HYPE {user}"
image = "hype.svg"
variant = "fullscreen"

[[counters]]
name = "muertes"
show_command = "muertes"
increment_command = "muertes+"
initial = 6
text = "Muertes: {value}"
variant = "counter"
duration_ms = 3500

[[event_triggers]]
name = "twitch-hype"
event = "redemption"
platforms = ["twitch"]
reward_title = "Hype"

[[event_triggers.steps]]
text = "{user} activó {command}: {args}"

[[event_triggers]]
name = "big-donation"
event = "donation"
min_amount = 10.0

[[event_triggers.emits]]
name = "camera_move"
payload = { preset = "desk" }
"#;

const INPUT: &[&str] = &[
    r#"{"id":"e1","platform":"twitch","type":"chat","user":{"id":"u1","name":"Ana","roles":["broadcaster"]},"data":{"message":"!hype"},"ts":"2026-11-04T10:00:00Z"}"#,
    r#"{"id":"e2","platform":"kick","type":"chat","user":{"id":"mod1","name":"Mod","roles":["moderator"]},"data":{"message":"!muertes+"},"ts":"2026-11-04T10:00:01Z"}"#,
    r#"{"id":"rota","platform":"#,
    r#"{"id":"e3","platform":"youtube","type":"chat","user":{"id":"u3","name":"Luz","roles":["subscriber"]},"data":{"message":"!hola mundo"},"ts":"2026-11-04T10:00:02Z"}"#,
    r#"{"id":"e4","platform":"twitch","type":"redemption","user":{"id":"u4","name":"Eva","roles":[]},"data":{"reward_id":"r1","reward_title":"Hype","cost":100,"input":"ya"},"ts":"2026-11-04T10:00:03Z"}"#,
    r#"{"id":"e5","platform":"youtube","type":"donation","user":{"id":"u5","name":"Leo","roles":[]},"data":{"amount":12.5,"currency":"USD","message":"gran stream"},"ts":"2026-11-04T10:00:04Z"}"#,
];

const EXPECTED: &[&str] = &[
    r#"{"type":"interaction_show","data":{"schema":1,"source":"mod_action","rule":"hype","step":0,"text":"HYPE Ana","image":"./assets/hype.svg","sound":null,"variant":"fullscreen","duration_ms":4000},"user":{"id":"u1","name":"Ana","roles":["broadcaster"]}}"#,
    r#"{"type":"interaction_counter","data":{"schema":1,"rule":"muertes","name":"muertes","value":7,"text":"Muertes: 7","image":null,"sound":null,"variant":"counter","duration_ms":3500},"user":{"id":"mod1","name":"Mod","roles":["moderator"]}}"#,
    r#"{"type":"interaction_show","data":{"schema":1,"source":"command","rule":"hola","step":0,"text":"Hola Luz: mundo","image":null,"sound":null,"variant":"default","duration_ms":3000},"user":{"id":"u3","name":"Luz","roles":["subscriber"]}}"#,
    r#"{"type":"interaction_show","data":{"schema":1,"source":"redemption","rule":"twitch-hype","step":0,"text":"Eva activó Hype: ya","image":null,"sound":null,"variant":"default","duration_ms":4000},"user":{"id":"u4","name":"Eva","roles":[]}}"#,
    r#"{"type":"interaction_emit","data":{"schema":1,"name":"camera_move","payload":{"preset":"desk"},"context":{"source":"donation","rule":"big-donation","command":"donation","args":"gran stream","count":1,"platform":"youtube"}},"user":{"id":"u5","name":"Leo","roles":[]}}"#,
];

fn plugin_root(name: &str, rules: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "interactions-protocol-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("overlay/assets")).unwrap();
    fs::write(root.join("config/interactions.toml"), rules).unwrap();
    fs::write(root.join("overlay/assets/hype.svg"), "<svg/>").unwrap();
    root
}

fn run_worker(root: &Path, lines: &[&str]) -> Output {
    let init = serde_json::json!({"type": "init", "config": {}, "data_dir": root.join("data")});
    let mut child = Command::new(env!("CARGO_BIN_EXE_interactions"))
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{init}").unwrap();
        for line in lines {
            let _ = writeln!(stdin, "{line}");
        }
    }
    child.wait_with_output().unwrap()
}

#[test]
fn worker_answers_jsonl_from_three_platforms() {
    let root = plugin_root("platforms", RULES);
    let output = run_worker(&root, INPUT);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stderr}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines, EXPECTED);
    for line in lines {
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }
    assert!(stderr.contains("JSON inválido"), "{stderr}");
    assert_eq!(
        fs::read_to_string(root.join("data/counters.json")).unwrap(),
        r#"{"version":1,"values":{"muertes":7}}"#
    );
}

#[test]
fn worker_fails_before_chat_when_assets_escape() {
    let root = plugin_root(
        "escape",
        "[[commands]]\nname = \"x\"\n\n[[commands.steps]]\ntext = \"x\"\nimage = \"../secret.svg\"\n",
    );
    let output = run_worker(&root, &[INPUT[0]]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("image"));
}
