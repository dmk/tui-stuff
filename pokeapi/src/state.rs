use serde::{Deserialize, Serialize};
use tui_dispatch_debug::debug::{DebugSection, DebugState, ron_string};

use crate::sprite::SpriteData;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokedexEntry {
    pub entry_number: u16,
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokemonDetail {
    pub id: u16,
    pub name: String,
    pub types: Vec<String>,
    pub stats: Vec<PokemonStat>,
    pub abilities: Vec<String>,
    pub moves: Vec<String>,
    pub height: u16,
    pub weight: u16,
    pub sprite_front_default: Option<String>,
    pub sprite_front_shiny: Option<String>,
    pub sprite_animated: Option<String>,
    pub cries_latest: Option<String>,
    pub cries_legacy: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokemonStat {
    pub name: String,
    pub value: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokemonSpecies {
    pub name: String,
    pub flavor_text: Option<String>,
    pub genus: Option<String>,
    pub evolution_chain_url: Option<String>,
    pub evolves_from: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvolutionChain {
    pub id: String,
    pub stages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoveDetail {
    pub name: String,
    pub power: Option<u16>,
    pub accuracy: Option<u16>,
    pub pp: Option<u16>,
    pub effect: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AbilityDetail {
    pub name: String,
    pub effect: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterLocation {
    pub location: String,
    pub version_details: Vec<EncounterVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterVersion {
    pub version: String,
    pub max_chance: u8,
    pub encounters: Vec<EncounterDetail>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncounterDetail {
    pub min_level: u8,
    pub max_level: u8,
    pub method: String,
    pub chance: u8,
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeMatchup {
    pub name: String,
    pub double_from: Vec<String>,
    pub half_from: Vec<String>,
    pub no_from: Vec<String>,
    pub double_to: Vec<String>,
    pub half_to: Vec<String>,
    pub no_to: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DetailMode {
    General,
    Move,
    Ability,
    Encounter,
    Matchup,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionInfo {
    pub name: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum FocusArea {
    Header,
    DexList,
    DetailTabs,
    Evolution,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub terminal_size: (u16, u16),
    pub focus: FocusArea,
    pub pokedex: Vec<PokedexEntry>,
    pub pokedex_all: Vec<PokedexEntry>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub detail_name: Option<String>,

    pub details: HashMap<String, PokemonDetail>,
    pub species: HashMap<String, PokemonSpecies>,
    pub evolution: HashMap<String, EvolutionChain>,
    pub sprite_cache: HashMap<String, SpriteData>,
    pub sprite_frame_index: usize,
    pub sprite_frame_tick: u64,
    pub move_cache: HashMap<String, MoveDetail>,
    pub ability_cache: HashMap<String, AbilityDetail>,
    pub encounter_cache: HashMap<String, Vec<EncounterLocation>>,
    pub type_matchup_cache: HashMap<String, TypeMatchup>,
    pub detail_mode: DetailMode,
    pub selected_move_index: usize,
    pub selected_ability_index: usize,
    pub selected_encounter_index: usize,
    pub evolution_selected_index: usize,

    pub search: SearchState,
    pub type_list: Vec<String>,
    pub type_filter: Option<String>,
    pub type_members: HashSet<String>,
    pub type_cache: HashMap<String, HashSet<String>>,

    pub regions: Vec<RegionInfo>,
    pub region_index: usize,
    pub seen: HashSet<String>,

    pub favorites: HashSet<String>,
    pub team: Vec<String>,

    pub list_loading: bool,
    pub detail_loading: bool,
    pub type_loading: bool,
    pub evolution_loading: bool,
    pub sprite_loading: bool,
    pub species_index_loading: bool,
    pub region_loading: bool,
    pub encounter_loading: bool,
    pub type_matchup_loading: bool,
    pub message: Option<String>,
    pub tick: u64,
    pub encounter_version_filter: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            terminal_size: (80, 24),
            focus: FocusArea::DexList,
            pokedex: Vec::new(),
            pokedex_all: Vec::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            detail_name: None,
            details: HashMap::new(),
            species: HashMap::new(),
            evolution: HashMap::new(),
            sprite_cache: HashMap::new(),
            sprite_frame_index: 0,
            sprite_frame_tick: 0,
            move_cache: HashMap::new(),
            ability_cache: HashMap::new(),
            encounter_cache: HashMap::new(),
            type_matchup_cache: HashMap::new(),
            detail_mode: DetailMode::General,
            selected_move_index: 0,
            selected_ability_index: 0,
            selected_encounter_index: 0,
            evolution_selected_index: 0,
            search: SearchState::default(),
            type_list: Vec::new(),
            type_filter: None,
            type_members: HashSet::new(),
            type_cache: HashMap::new(),
            regions: Vec::new(),
            region_index: 0,
            seen: HashSet::new(),
            favorites: HashSet::new(),
            team: Vec::new(),
            list_loading: false,
            detail_loading: false,
            type_loading: false,
            evolution_loading: false,
            sprite_loading: false,
            species_index_loading: false,
            region_loading: false,
            encounter_loading: false,
            type_matchup_loading: false,
            message: None,
            tick: 0,
            encounter_version_filter: None,
        }
    }
}

impl AppState {
    pub fn selected_entry(&self) -> Option<&PokedexEntry> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|idx| self.pokedex.get(*idx))
    }

    pub fn selected_name(&self) -> Option<String> {
        self.selected_entry().map(|entry| entry.name.clone())
    }

    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if self.filtered_indices.is_empty() {
            self.selected_index = 0;
            return false;
        }
        let bounded = index.min(self.filtered_indices.len() - 1);
        if bounded != self.selected_index {
            self.selected_index = bounded;
            return true;
        }
        false
    }

    pub fn rebuild_filtered(&mut self) {
        let query = self.search.query.trim().to_lowercase();
        let type_filter = self.type_filter.clone();
        self.filtered_indices = self
            .pokedex
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                let matches_query = query.is_empty()
                    || entry.name.to_lowercase().contains(&query)
                    || entry.entry_number.to_string().contains(&query);
                let matches_type = match &type_filter {
                    Some(_) => self.type_members.contains(&entry.name),
                    None => true,
                };
                matches_query && matches_type
            })
            .map(|(idx, _)| idx)
            .collect();

        if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = 0;
        }
    }

    pub fn update_type_members(&mut self, type_name: &str, pokemon: HashSet<String>) {
        self.type_cache.insert(type_name.to_string(), pokemon.clone());
        if self.type_filter.as_deref() == Some(type_name) {
            self.type_members = pokemon;
        }
    }

    pub fn current_detail(&self) -> Option<&PokemonDetail> {
        let name = self.detail_name.as_ref()?;
        self.details.get(name)
    }

    pub fn current_species(&self) -> Option<&PokemonSpecies> {
        let name = self.detail_name.as_ref()?;
        self.species.get(name)
    }

    pub fn current_region(&self) -> Option<&RegionInfo> {
        self.regions.get(self.region_index)
    }

    pub fn current_move_name(&self) -> Option<String> {
        let detail = self.current_detail()?;
        detail
            .moves
            .get(self.selected_move_index)
            .cloned()
    }

    pub fn current_ability_name(&self) -> Option<String> {
        let detail = self.current_detail()?;
        detail
            .abilities
            .get(self.selected_ability_index)
            .cloned()
    }

    pub fn reset_sprite_animation(&mut self) {
        self.sprite_frame_index = 0;
        self.sprite_frame_tick = 0;
    }

    pub fn reset_detail_selection(&mut self) {
        self.detail_mode = DetailMode::General;
        self.selected_move_index = 0;
        self.selected_ability_index = 0;
        self.selected_encounter_index = 0;
    }

    pub fn focus_next(&mut self) {
        self.focus = match self.focus {
            FocusArea::Header => FocusArea::DexList,
            FocusArea::DexList => FocusArea::DetailTabs,
            FocusArea::DetailTabs => FocusArea::Evolution,
            FocusArea::Evolution => FocusArea::DexList,
        };
    }

    pub fn focus_prev(&mut self) {
        self.focus = match self.focus {
            FocusArea::Header => FocusArea::Evolution,
            FocusArea::DexList => FocusArea::Evolution,
            FocusArea::DetailTabs => FocusArea::DexList,
            FocusArea::Evolution => FocusArea::DetailTabs,
        };
    }
}

impl DebugState for AppState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        vec![
            DebugSection::new("Dex")
                .entry("total", ron_string(&self.pokedex.len()))
                .entry("filtered", ron_string(&self.filtered_indices.len()))
                .entry("selected", ron_string(&self.selected_index))
                .entry("detail", ron_string(&self.detail_name))
                .entry("region", ron_string(&self.current_region().map(|region| region.label.clone()))),
            DebugSection::new("Filters")
                .entry("search", ron_string(&self.search.query))
                .entry("search_active", ron_string(&self.search.active))
                .entry("type", ron_string(&self.type_filter))
                .entry("detail_mode", ron_string(&self.detail_mode))
                .entry("focus", ron_string(&self.focus))
                .entry("evolution_index", ron_string(&self.evolution_selected_index))
                .entry(
                    "encounter_index",
                    ron_string(&self.selected_encounter_index),
                )
                .entry(
                    "encounter_version",
                    ron_string(&self.encounter_version_filter),
                ),
            DebugSection::new("Status")
                .entry("list_loading", ron_string(&self.list_loading))
                .entry("detail_loading", ron_string(&self.detail_loading))
                .entry("sprite_loading", ron_string(&self.sprite_loading))
                .entry("species_index_loading", ron_string(&self.species_index_loading))
                .entry("encounter_loading", ron_string(&self.encounter_loading))
                .entry("matchup_loading", ron_string(&self.type_matchup_loading))
                .entry("region_loading", ron_string(&self.region_loading))
                .entry("message", ron_string(&self.message)),
        ]
    }
}
