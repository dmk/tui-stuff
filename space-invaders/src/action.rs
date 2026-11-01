use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Direction {
    Left,
    Right,
}

/// Keep aliases separate so releasing A does not cancel a still-held Left key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MovementKey {
    Left,
    A,
    Right,
    D,
}

impl MovementKey {
    pub fn mask(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::A => 2,
            Self::Right => 4,
            Self::D => 8,
        }
    }
}

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[action(infer_categories)]
pub enum Action {
    GameStart,
    PlayerMove(Direction),
    PlayerKeyDown(MovementKey),
    PlayerKeyUp(MovementKey),
    PlayerControlsClear,
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
