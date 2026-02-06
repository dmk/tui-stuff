use crate::state::SpriteTarget;

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    LoadPokemon { target: SpriteTarget, name: String },
    LoadSprite { target: SpriteTarget, url: String },
    PlayAttackSound,
}
