use crate::llm::schema::{action_schema_string, dialogue_schema_string};
use crate::llm::{ChatMessage, LlmRequest};
use crate::state::{AppState, DialogueLine, NpcState};

const HISTORY_LIMIT: usize = 6;

pub fn build_dialogue_request(state: &AppState, npc: &NpcState, player_text: &str) -> LlmRequest {
    let system = format!(
        "You are an NPC in a rules-driven fantasy game.\n\
Respond ONLY with a single JSON object matching this schema:\n{}\n\n\
Return strict JSON: use double quotes, no trailing commas, no markdown, no backticks, no extra text.\n\n\
NPC name: {}\nPersona: {}\nDialogue notes: {}\n\n\
Setting lore: {}\n",
        dialogue_schema_string(),
        npc.name,
        npc.persona,
        npc.dialogue_prompt,
        format_lore(state),
    );

    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: system,
    }];

    for line in recent_dialogue(&state.dialogue.history) {
        messages.push(ChatMessage {
            role: line.speaker.clone(),
            content: line.text.clone(),
        });
    }

    messages.push(ChatMessage {
        role: "user".to_string(),
        content: player_text.to_string(),
    });

    LlmRequest {
        id: state.rng_seed,
        messages,
        stream: false,
    }
}

pub fn build_action_request(state: &AppState, player_text: &str) -> LlmRequest {
    let system = format!(
        "You interpret player actions into a single rules check.\n\
Return ONLY a single JSON object matching this schema:\n{}\n\n\
Return strict JSON: use double quotes, no trailing commas, no markdown, no backticks, no extra text.\n\n\
Always set \"kind\" to \"skill_check\".\n\
Allowed skills: athletics, acrobatics, stealth, perception, persuasion, arcana.\n\
Allowed abilities: strength, dexterity, constitution, intelligence, wisdom, charisma.\n\
Difficulties: easy, medium, hard.\n\n\
Setting lore: {}\n\
Player location: {}\n",
        action_schema_string(),
        format_lore(state),
        state.map.name,
    );

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system,
        },
        ChatMessage {
            role: "user".to_string(),
            content: player_text.to_string(),
        },
    ];

    LlmRequest {
        id: state.rng_seed,
        messages,
        stream: false,
    }
}

fn recent_dialogue(lines: &[DialogueLine]) -> Vec<DialogueLine> {
    if lines.len() <= HISTORY_LIMIT {
        return lines.to_vec();
    }
    lines[lines.len() - HISTORY_LIMIT..].to_vec()
}

fn format_lore(state: &AppState) -> String {
    state
        .scenario
        .as_ref()
        .map(|s| s.lore.join(" | "))
        .unwrap_or_else(|| "(none)".to_string())
}
