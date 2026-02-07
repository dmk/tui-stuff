use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::llm::schema::ActionInterpretation;
use crate::scenario::ScenarioRuntime;
use crate::state::{AppState, Direction};

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[action(infer_categories)]
pub enum Action {
    Init,
    UiTerminalResize(u16, u16),
    UiRender,
    Tick,

    Move(Direction),
    Interact,
    Talk,
    OpenInventory,
    InventorySelect(usize),
    OpenCustomAction,
    CloseOverlay,
    MenuSelect(usize),
    MenuConfirm,
    SaveExists(bool),
    PauseOpen,
    PauseClose,
    PauseSelect(usize),
    PauseConfirm,

    DialogueInputChanged(String),
    DialogueSubmit,
    DialogueResponse { npc_id: String, line: String },

    CustomActionInputChanged(String),
    CustomActionSubmit,
    CustomActionInterpreted(ActionInterpretation),

    CombatAttack,
    CombatEndTurn,

    ScrollLog(i16),

    CreationNameChanged(String),
    CreationSelectClass(usize),
    CreationSelectBackground(usize),
    CreationSelectStat(usize),
    CreationAdjustStat(i8),
    CreationNext,
    CreationBack,
    CreationConfirm,

    SaveComplete,
    SaveError(String),
    LoadComplete(Box<AppState>),
    LoadError(String),

    ScenarioLoaded { scenario: ScenarioRuntime },
    ScenarioLoadError { error: String },

    LlmError(String),

    Quit,
}
