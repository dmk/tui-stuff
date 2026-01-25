#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    LoadPokedex { name: String },
    LoadRegions,
    LoadSpeciesIndex { names: Vec<String> },
    LoadTypes,
    LoadTypeDetail { name: String },
    LoadPokemonDetail { name: String },
    LoadPokemonSpecies { name: String },
    LoadEncounters { name: String },
    LoadTypeMatchup { name: String },
    LoadEvolutionChain { id: String, url: String },
    LoadSprite { name: String, url: String },
    PlayCry { name: String, url: String },
    LoadMoveDetail { name: String },
    LoadAbilityDetail { name: String },
}
