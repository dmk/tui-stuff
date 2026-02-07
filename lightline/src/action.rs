use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Direction, GeneratedFloor};

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[action(infer_categories)]
pub enum Action {
    Init,
    GenerateFloor,
    FloorGenerated(GeneratedFloor),

    Move(Direction, bool),
    Interact,
    Tick,
    DangerAdvance,
    Descend,
    GameOver,

    PauseOpen,
    PauseClose,

    Quit,
}
