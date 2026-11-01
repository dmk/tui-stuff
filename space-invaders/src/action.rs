use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Direction {
    Left,
    Right,
}

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[action(infer_categories)]
pub enum Action {
    GameStart,
    PlayerMove(Direction),
    PlayerFire,
    PauseToggle,
    GameRestart,
    Tick,
    /// A deterministic game-time wait; exactly equivalent to this many Ticks.
    GameAdvanceTicks(u32),
    /// Terminal availability belongs in the action stream, never in rendering.
    ViewportResize {
        width: u16,
        height: u16,
    },
    Quit,
}
