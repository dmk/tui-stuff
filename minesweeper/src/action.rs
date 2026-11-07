use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tui_dispatch::prelude::*;

use crate::state::Difficulty;

#[derive(Action, Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum Action {
    Quit,
    Reveal(usize, usize),
    RevealWave,
    ToggleFlag(usize, usize),
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    SetDifficulty(Difficulty),
    NewGame,
    CustomOpen,
    CustomCancel,
    CustomField(i8),
    CustomValue(String),
    CustomCursorMove,
    CustomConfirm,
}
