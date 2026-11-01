use game_of_life::state::AppState;
use std::{
    path::PathBuf,
    process::{Command, Output},
};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("life-cli-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().into()
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_game-of-life"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn json_actions_produce_a_frame_and_recordings_replay_exactly() {
    let temp = TempDir::new();
    let input = temp.file("actions.json");
    let recorded = temp.file("recorded.json");
    let first = temp.file("first.json");
    let second = temp.file("second.json");
    let schema = temp.file("schema.json");
    std::fs::write(&input, r#"["Randomize", "PauseToggle", "Tick", "Tick", "Tick", "Tick", "Tick", "Tick", "Tick", {"Advance":18}]"#).unwrap();
    let output = run(&[
        "--debug-render-once",
        "--seed",
        "0",
        "--debug-actions-in",
        &input,
        "--debug-actions-out",
        &recorded,
        "--debug-state-out",
        &first,
        "--debug-actions-schema-out",
        &schema,
    ]);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("PAUSED"));
    assert!(text.contains("000019"));
    assert!(text.contains("PRIMORDIAL SOUP"));
    assert_eq!(text.lines().count(), 42);
    assert!(!text.contains('\u{1b}'));
    let recorded_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&recorded).unwrap()).unwrap();
    assert!(recorded_json
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action == "Tick"));
    let replay = run(&[
        "--debug-render-once",
        "--seed",
        "0",
        "--debug-actions-in",
        &recorded,
        "--debug-state-out",
        &second,
    ]);
    assert_eq!(output.stdout, replay.stdout);
    assert_eq!(
        std::fs::read_to_string(&first).unwrap(),
        std::fs::read_to_string(&second).unwrap()
    );
    let state: AppState = serde_json::from_str(&std::fs::read_to_string(&first).unwrap()).unwrap();
    state.validate().unwrap();
    let snapshot = run(&["--debug-render-once", "--debug-state-in", &first]);
    assert_eq!(output.stdout, snapshot.stdout);
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&schema).unwrap()).unwrap();
    assert!(schema.get("$schema").is_some());
}

#[test]
fn supplied_scenes_work_at_compact_and_large_dimensions() {
    for scene in ["garden", "gun", "soup", "editor"] {
        for (width, height) in [("80", "24"), ("176", "60")] {
            let output = run(&[
                "--debug-render-once",
                "--width",
                width,
                "--height",
                height,
                "--debug-actions-in",
                &format!("scenarios/{scene}.json"),
            ]);
            assert!(String::from_utf8_lossy(&output.stdout).contains("PAUSED"));
        }
    }
}
