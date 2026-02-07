use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use crate::state::{AppState, LogEntry};

pub async fn save_game(state: &AppState, since: usize) -> Result<(), String> {
    let path = PathBuf::from(&state.save_path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create save directory: {}", e))?;
    }
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize state: {}", e))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("Failed to write save file: {}", e))?;

    append_transcript(state, since).await?;
    Ok(())
}

pub async fn load_game(path: &str) -> Result<AppState, String> {
    let json = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read save file: {}", e))?;
    serde_json::from_str(&json).map_err(|e| format!("Save file corrupted: {}", e))
}

async fn append_transcript(state: &AppState, since: usize) -> Result<(), String> {
    let path = transcript_path(&state.save_path);
    let entries = state.log.iter().skip(since).collect::<Vec<&LogEntry>>();
    if entries.is_empty() {
        return Ok(());
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|e| format!("Failed to open transcript: {}", e))?;

    for entry in entries {
        let line = serde_json::to_string(entry)
            .map_err(|e| format!("Failed to serialize transcript entry: {}", e))?;
        file.write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Failed to write transcript: {}", e))?;
        file.write_all(b"\n")
            .await
            .map_err(|e| format!("Failed to write transcript: {}", e))?;
    }

    Ok(())
}

fn transcript_path(save_path: &str) -> PathBuf {
    let path = PathBuf::from(save_path);
    if let Some(dir) = path.parent() {
        return dir.join("transcript.jsonl");
    }
    PathBuf::from("transcript.jsonl")
}
