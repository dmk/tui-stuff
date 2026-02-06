use serde::{Deserialize, Serialize};

use crate::sprite::SpriteData;
use crate::state::{AppState, Direction, PokemonInfo, SpriteTarget};

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[action(infer_categories)]
pub enum Action {
    Init,
    UiTerminalResize(u16, u16),
    Tick,
    Move(Direction),

    // Battle actions
    BattleMenuNext,
    BattleMenuPrev,
    BattleConfirm,
    BattleItemCancel,

    // Main menu actions
    MenuSelect(usize),
    MenuConfirm,
    SaveExists(bool),

    // Pokemon selection actions
    StarterSelect(usize),
    StarterConfirm,
    StarterPreviewLoaded {
        info: PokemonInfo,
    },
    StarterPreviewSpriteLoaded {
        sprite: SpriteData,
    },
    StarterPreviewError {
        error: String,
    },

    // Pause menu actions
    PauseOpen,
    PauseClose,
    PauseSelect(usize),
    PauseConfirm,

    // Save/Load actions
    SaveGame,
    SaveComplete,
    SaveError(String),
    LoadGame,
    LoadComplete(Box<AppState>),
    LoadError(String),

    // Pokemon loading
    PokemonDidLoad {
        target: SpriteTarget,
        info: PokemonInfo,
    },
    PokemonDidError {
        target: SpriteTarget,
        name: String,
        error: String,
    },
    SpriteDidLoad {
        target: SpriteTarget,
        sprite: SpriteData,
    },
    SpriteDidError {
        target: SpriteTarget,
        error: String,
    },

    Quit,
}
