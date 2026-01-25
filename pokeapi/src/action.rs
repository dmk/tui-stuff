use serde::{Deserialize, Serialize};

use crate::sprite::SpriteData;
use crate::state::{
    AbilityDetail, EncounterLocation, EvolutionChain, FocusArea, MoveDetail, PokemonDetail,
    PokemonSpecies, PokedexEntry, RegionInfo, TypeMatchup,
};

#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[action(infer_categories)]
pub enum Action {
    Init,
    PokedexDidLoad(Vec<PokedexEntry>),
    PokedexDidError(String),

    SpeciesIndexDidLoad(Vec<PokemonSpecies>),
    SpeciesIndexDidError(String),

    RegionsDidLoad(Vec<RegionInfo>),
    RegionsDidError(String),
    RegionNext,
    RegionPrev,

    FocusNext,
    FocusPrev,
    FocusSet(FocusArea),

    TypesDidLoad(Vec<String>),
    TypesDidError(String),
    TypeFilterNext,
    TypeFilterPrev,
    TypeFilterClear,
    TypeFilterDidLoad { name: String, pokemon: Vec<String> },
    TypeFilterDidError { name: String, error: String },

    SelectionMove(i16),
    SelectionPage(i16),
    SelectionJumpTop,
    SelectionJumpBottom,
    DexSelect(usize),

    SearchStart,
    SearchCancel,
    SearchSubmit,
    SearchInput(char),
    SearchBackspace,

    PokemonDidLoad(PokemonDetail),
    PokemonDidError { name: String, error: String },
    PokemonSpeciesDidLoad(PokemonSpecies),
    PokemonSpeciesDidError { name: String, error: String },
    EvolutionDidLoad { id: String, chain: EvolutionChain },
    EvolutionDidError { id: String, error: String },
    EvolutionSelect(usize),
    SpriteDidLoad { name: String, sprite: SpriteData },
    SpriteDidError { name: String, error: String },
    MoveDetailDidLoad(MoveDetail),
    MoveDetailDidError { name: String, error: String },
    AbilityDetailDidLoad(AbilityDetail),
    AbilityDetailDidError { name: String, error: String },
    EncounterDidLoad { name: String, encounters: Vec<EncounterLocation> },
    EncounterDidError { name: String, error: String },
    TypeMatchupDidLoad { name: String, matchup: TypeMatchup },
    TypeMatchupDidError { name: String, error: String },

    DetailModeToggle,
    DetailTabNext,
    DetailTabPrev,
    DetailNext,
    DetailPrev,
    MoveSelect(usize),
    AbilitySelect(usize),
    EncounterSelect(usize),
    EncounterFilterNext,
    EncounterFilterPrev,

    ToggleFavorite,
    ToggleTeam,
    PlayCry,
    CryDidError(String),

    UiTerminalResize(u16, u16),
    Tick,
    Quit,
}
