use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::patterns::Pattern;

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[action(infer_categories)]
pub enum Action {
    Tick,
    PauseToggle,
    Step,
    /// Advance generations directly, independently of wall time and pause.
    Advance(u32),
    SpeedChange(i8),
    PatternSelect(Pattern),
    /// Replace the world with the selected pattern, centered in the world.
    PatternLoad,
    /// Plant the selected pattern at the editor cursor.
    PatternStamp,
    CellSet {
        x: i32,
        y: i32,
        alive: bool,
    },
    CellToggle,
    CursorMove {
        dx: i32,
        dy: i32,
    },
    CameraPan {
        dx: i32,
        dy: i32,
    },
    CameraHome,
    ZoomCycle,
    Clear,
    Randomize,
    BoundaryToggle,
    GridToggle,
    TrailsToggle,
    HelpToggle,
    ViewportResize {
        width: u16,
        height: u16,
    },
    Quit,
}
