use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use space_invaders::state::{AppState, Phase};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "invaders-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_space-invaders"))
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
fn scripted_cli_prints_final_frame_and_replays_recorded_ticks() {
    let temp = TempDir::new();
    let input = temp.file("input.json");
    let recorded = temp.file("recorded.json");
    let first_state = temp.file("first.json");
    let replayed_state = temp.file("replayed.json");
    std::fs::write(
        &input,
        r#"["GameStart", "Tick", "PlayerFire", {"GameAdvanceTicks":50}, "Tick", "PauseToggle"]"#,
    )
    .unwrap();
    let first = run(&[
        "--debug-render-once",
        "--debug-actions-in",
        &input,
        "--debug-actions-out",
        &recorded,
        "--debug-state-out",
        &first_state,
    ]);
    let frame = String::from_utf8_lossy(&first.stdout);
    assert!(frame.contains("PAUSED"));
    assert_eq!(frame.lines().count(), 40);
    assert!(!frame.contains('\u{1b}'), "headless output is plain text");
    let actions: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&recorded).unwrap()).unwrap();
    assert!(actions
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action == "Tick"));
    let second = run(&[
        "--debug-render-once",
        "--debug-actions-in",
        &recorded,
        "--debug-state-out",
        &replayed_state,
    ]);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        std::fs::read_to_string(first_state).unwrap(),
        std::fs::read_to_string(replayed_state).unwrap()
    );
}

#[test]
fn headless_dimensions_choose_all_sprite_sizes_and_suspend_too_small() {
    let temp = TempDir::new();
    let state_path = temp.file("state.json");
    for (width, height) in [("80", "40"), ("112", "59"), ("176", "78")] {
        let output = run(&[
            "--debug-render-once",
            "--sprite-size",
            "auto",
            "--debug-actions-in",
            "scenarios/formation.json",
            "--width",
            width,
            "--height",
            height,
            "--debug-state-out",
            &state_path,
        ]);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().lines().count(),
            height.parse::<usize>().unwrap()
        );
        let state: AppState =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        assert_eq!(state.tick, 46);
        assert_eq!(state.phase, Phase::Playing);
    }
    let output = run(&[
        "--debug-render-once",
        "--debug-actions-in",
        "scenarios/formation.json",
        "--width",
        "63",
        "--debug-state-out",
        &state_path,
    ]);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Resize to at least"));
    let state: AppState =
        serde_json::from_str(&std::fs::read_to_string(state_path).unwrap()).unwrap();
    assert_eq!(state.tick, 0);
}

#[test]
fn framework_await_markers_and_schema_exports_work() {
    let temp = TempDir::new();
    let state_path = temp.file("state.json");
    let schema = temp.file("state-schema.json");
    let actions = temp.file("actions-schema.json");
    run(&[
        "--debug-render-once",
        "--debug-actions-in",
        "scenarios/await-start.json",
        "--debug-state-out",
        &state_path,
        "--debug-state-schema-out",
        &schema,
        "--debug-actions-schema-out",
        &actions,
        "--debug-replay-timeout",
        "1",
    ]);
    let state: AppState =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(state.tick, 8);
    assert_eq!(state.phase, Phase::Playing);
    for path in [&schema, &actions] {
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(json.get("$schema").is_some());
    }
    let output = run(&["--debug-render-once", "--debug-state-in", &state_path]);
    assert!(String::from_utf8(output.stdout).unwrap().contains("SCORE"));
}

#[test]
fn default_headless_frame_is_the_title_screen() {
    let output = run(&["--debug-render-once"]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("SPACE INVADERS"));
    assert!(text.contains("ENTER  launch"));
}

#[test]
fn default_sizing_grows_to_fit_but_compact_can_be_requested() {
    let args = [
        "--debug-render-once",
        "--width",
        "176",
        "--height",
        "78",
        "--debug-actions-in",
        "scenarios/formation.json",
    ];
    let auto = String::from_utf8(run(&args).stdout).unwrap();
    assert!(
        auto.contains("▟█▀█▀█▀█▙"),
        "large screen should select large sprites by default"
    );
    let mut compact_args = args.to_vec();
    compact_args.extend(["--sprite-size", "compact"]);
    let compact = String::from_utf8(run(&compact_args).stdout).unwrap();
    assert!(compact.contains("▘▀ ▀▝"));
    assert!(!compact.contains("▟█▀█▀█▀█▙"));
}
