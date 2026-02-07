use crate::state::{AppState, SpriteTarget};

#[derive(Clone, Debug)]
pub enum Effect {
    LoadPokemon { target: SpriteTarget, name: String },
    LoadSprite { target: SpriteTarget, url: String },
    PlayAttackSound,
    LoadScenario { path: String },
    LoadPartySprite { index: usize, url: String },

    // Save/Load
    CheckSaveExists,
    SaveGame { state: Box<AppState> },
    LoadGame,

    // Starter preview
    LoadStarterPreview { name: String },
    LoadStarterSprite { url: String },
}
