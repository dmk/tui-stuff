use serde::{Deserialize, Serialize};

use crate::sprite::SpriteData;
use crate::state::{Direction, PokemonInfo, SpriteTarget};

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[action(infer_categories)]
pub enum Action {
    Init,
    UiTerminalResize(u16, u16),
    Tick,
    Move(Direction),
    BattleMenuNext,
    BattleMenuPrev,
    BattleConfirm,
    PokemonDidLoad { target: SpriteTarget, info: PokemonInfo },
    PokemonDidError { target: SpriteTarget, name: String, error: String },
    SpriteDidLoad { target: SpriteTarget, sprite: SpriteData },
    SpriteDidError { target: SpriteTarget, error: String },
    Quit,
}
