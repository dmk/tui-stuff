use crate::llm::LlmRequest;
use crate::state::AppState;

#[derive(Clone, Debug)]
pub enum Effect {
    CallLlmDialogue { npc_id: String, request: LlmRequest },
    CallLlmInterpretAction { request: LlmRequest },
    SaveGame { state: Box<AppState>, since: usize },
    LoadGame { path: String },
    LoadScenario { path: String },
    CheckSaveExists { path: String },
}
