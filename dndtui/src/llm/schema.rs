use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DialogueResponse {
    pub npc_line: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActionInterpretation {
    pub kind: String,
    pub skill: String,
    pub difficulty: String,
    pub reason: String,
    pub on_success: String,
    pub on_failure: String,
}

pub fn dialogue_schema_string() -> String {
    let schema = schemars::schema_for!(DialogueResponse);
    serde_json::to_string_pretty(&schema.schema).unwrap_or_else(|_| "{}".to_string())
}

pub fn action_schema_string() -> String {
    let schema = schemars::schema_for!(ActionInterpretation);
    serde_json::to_string_pretty(&schema.schema).unwrap_or_else(|_| "{}".to_string())
}

pub fn parse_json_loose<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty response".to_string());
    }

    if let Ok(parsed) = serde_json::from_str::<T>(trimmed) {
        return Ok(parsed);
    }

    if let Ok(inner) = serde_json::from_str::<String>(trimmed) {
        if let Ok(parsed) = serde_json::from_str::<T>(&inner) {
            return Ok(parsed);
        }
    }

    if let Some(candidate) = extract_json_candidate(trimmed) {
        if let Ok(parsed) = serde_json::from_str::<T>(&candidate) {
            return Ok(parsed);
        }
    }

    Err(format!("invalid JSON: {}", shorten(trimmed, 200)))
}

pub fn parse_dialogue_response(raw: &str) -> Result<DialogueResponse, String> {
    if let Ok(parsed) = parse_json_loose::<DialogueResponse>(raw) {
        return Ok(parsed);
    }
    if let Some(line) = extract_field(raw, "npc_line") {
        return Ok(DialogueResponse { npc_line: line });
    }
    Err("invalid dialogue response".to_string())
}

pub fn parse_action_interpretation(raw: &str) -> Result<ActionInterpretation, String> {
    if let Ok(parsed) = parse_json_loose::<ActionInterpretation>(raw) {
        return Ok(parsed);
    }
    let skill = extract_field_any(raw, &["skill", "ability"]);
    let difficulty = extract_field(raw, "difficulty");
    let reason = extract_field(raw, "reason");
    let on_success = extract_field_any(raw, &["on_success", "success"]);
    let on_failure = extract_field_any(raw, &["on_failure", "failure"]);

    if skill.is_none()
        && difficulty.is_none()
        && reason.is_none()
        && on_success.is_none()
        && on_failure.is_none()
    {
        return Err("invalid action response".to_string());
    }

    Ok(ActionInterpretation {
        kind: extract_field(raw, "kind").unwrap_or_else(|| "skill_check".to_string()),
        skill: skill.unwrap_or_else(|| "strength".to_string()),
        difficulty: difficulty.unwrap_or_else(|| "medium".to_string()),
        reason: reason.unwrap_or_else(|| "Action".to_string()),
        on_success: on_success.unwrap_or_else(|| "You succeed.".to_string()),
        on_failure: on_failure.unwrap_or_else(|| "You fail.".to_string()),
    })
}

fn extract_json_candidate(raw: &str) -> Option<String> {
    if let Some(fenced) = extract_fenced_json(raw) {
        return Some(fenced);
    }
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(raw[start..=end].to_string())
}

fn extract_fenced_json(raw: &str) -> Option<String> {
    let fence = "```";
    let start = raw.find(fence)?;
    let rest = &raw[start + fence.len()..];
    let end = rest.find(fence)?;
    let mut inside = rest[..end].trim();
    if inside.starts_with("json") {
        inside = inside["json".len()..].trim_start();
    }
    if inside.is_empty() {
        None
    } else {
        Some(inside.to_string())
    }
}

fn extract_field(raw: &str, key: &str) -> Option<String> {
    for pattern in [
        format!("\"{}\"", key),
        format!("'{}'", key),
        key.to_string(),
    ] {
        if let Some(pos) = raw.find(&pattern) {
            let after = &raw[pos + pattern.len()..];
            let colon = after.find(':')?;
            let mut rest = after[colon + 1..].trim_start();
            if rest.is_empty() {
                continue;
            }
            let first = rest.chars().next()?;
            if first == '"' || first == '\'' {
                rest = &rest[1..];
                let mut out = String::new();
                let mut escaped = false;
                for ch in rest.chars() {
                    if escaped {
                        out.push(ch);
                        escaped = false;
                        continue;
                    }
                    if ch == '\\' {
                        escaped = true;
                        continue;
                    }
                    if ch == first {
                        return Some(out);
                    }
                    out.push(ch);
                }
            } else {
                let end = rest
                    .find(|c: char| c == ',' || c == '}' || c == '\n' || c == '\r')
                    .unwrap_or(rest.len());
                let value = rest[..end].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn extract_field_any(raw: &str, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = extract_field(raw, key) {
            return Some(value);
        }
    }
    None
}

fn shorten(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut out = text.chars().take(max).collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dialogue() {
        let input = r#"{"npc_line":"Hello."}"#;
        let response: DialogueResponse = parse_json_loose(input).unwrap();
        assert_eq!(response.npc_line, "Hello.");
    }

    #[test]
    fn parse_action() {
        let input = r#"{"kind":"skill_check","skill":"athletics","difficulty":"medium","reason":"Leap","on_success":"You clear it.","on_failure":"You slip."}"#;
        let response: ActionInterpretation = parse_json_loose(input).unwrap();
        assert_eq!(response.skill, "athletics");
    }

    #[test]
    fn parse_fenced_json() {
        let input = "```json\n{\"npc_line\":\"Hi.\"}\n```";
        let response: DialogueResponse = parse_json_loose(input).unwrap();
        assert_eq!(response.npc_line, "Hi.");
    }

    #[test]
    fn parse_dialogue_fallback() {
        let input = "npc_line: 'Hey there'";
        let response = parse_dialogue_response(input).unwrap();
        assert_eq!(response.npc_line, "Hey there");
    }
}
