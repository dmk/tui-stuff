use tui_dispatch::DispatchResult;

use std::collections::HashSet;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::AppState;

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Init => {
            state.list_loading = true;
            state.type_loading = true;
            state.region_loading = true;
            state.species_index_loading = true;
            state.message = None;
            DispatchResult::changed_with_many(vec![
                Effect::LoadRegions,
                Effect::LoadPokedex {
                    name: "kanto".to_string(),
                },
                Effect::LoadTypes,
            ])
        }

        Action::PokedexDidLoad(entries) => {
            state.pokedex_all = entries;
            state.pokedex.clear();
            state.filtered_indices.clear();
            state.selected_index = 0;
            state.detail_name = None;
            state.list_loading = false;
            state.species_index_loading = true;
            state.reset_sprite_animation();
            state.reset_detail_selection();
            let names = state
                .pokedex_all
                .iter()
                .map(|entry| entry.name.clone())
                .collect();
            DispatchResult::changed_with(Effect::LoadSpeciesIndex { names })
        }

        Action::PokedexDidError(error) => {
            state.list_loading = false;
            state.species_index_loading = false;
            state.message = Some(format!("Pokedex error: {error}"));
            DispatchResult::changed()
        }

        Action::SpeciesIndexDidLoad(species_list) => {
            state.species_index_loading = false;
            for species in species_list {
                state.species.insert(species.name.clone(), species);
            }
            let region_species: HashSet<String> = state
                .pokedex_all
                .iter()
                .map(|entry| entry.name.clone())
                .collect();
            state.pokedex = state
                .pokedex_all
                .iter()
                .filter(|entry| {
                    state
                        .species
                        .get(&entry.name)
                        .map(|species| {
                            species
                                .evolves_from
                                .as_deref()
                                .map(|ancestor| !region_species.contains(ancestor))
                                .unwrap_or(true)
                        })
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            state.rebuild_filtered();
            state.selected_index = 0;
            let effects = select_current(state);
            if effects.is_empty() {
                DispatchResult::changed()
            } else {
                DispatchResult::changed_with_many(effects)
            }
        }

        Action::SpeciesIndexDidError(error) => {
            state.species_index_loading = false;
            state.message = Some(format!("Species index error: {error}"));
            state.pokedex = state.pokedex_all.clone();
            state.rebuild_filtered();
            state.selected_index = 0;
            let effects = select_current(state);
            if effects.is_empty() {
                DispatchResult::changed()
            } else {
                DispatchResult::changed_with_many(effects)
            }
        }

        Action::RegionsDidLoad(regions) => {
            state.region_loading = false;
            state.regions = regions;
            if let Some(index) = state
                .regions
                .iter()
                .position(|region| region.name == "kanto")
            {
                state.region_index = index;
            } else if state.region_index >= state.regions.len() {
                state.region_index = 0;
            }
            DispatchResult::changed()
        }

        Action::RegionsDidError(error) => {
            state.region_loading = false;
            state.message = Some(format!("Region error: {error}"));
            DispatchResult::changed()
        }

        Action::RegionNext => cycle_region(state, 1),
        Action::RegionPrev => cycle_region(state, -1),

        Action::FocusNext => {
            if state.search.active {
                return DispatchResult::unchanged();
            }
            state.focus_next();
            DispatchResult::changed()
        }

        Action::FocusPrev => {
            if state.search.active {
                return DispatchResult::unchanged();
            }
            state.focus_prev();
            DispatchResult::changed()
        }

        Action::FocusSet(area) => {
            if state.search.active {
                return DispatchResult::unchanged();
            }
            if state.focus == area {
                return DispatchResult::unchanged();
            }
            state.focus = area;
            DispatchResult::changed()
        }

        Action::TypesDidLoad(types) => {
            state.type_loading = false;
            state.type_list = types;
            DispatchResult::changed()
        }

        Action::TypesDidError(error) => {
            state.type_loading = false;
            state.message = Some(format!("Type error: {error}"));
            DispatchResult::changed()
        }

        Action::TypeFilterNext => cycle_filter(state, 1),
        Action::TypeFilterPrev => cycle_filter(state, -1),

        Action::TypeFilterClear => {
            if state.type_filter.is_none() {
                return DispatchResult::unchanged();
            }
            state.type_filter = None;
            state.type_members.clear();
            state.rebuild_filtered();
            let effects = select_current(state);
            DispatchResult::changed_with_many(effects)
        }

        Action::TypeFilterDidLoad { name, pokemon } => {
            let set = pokemon.into_iter().collect();
            state.update_type_members(&name, set);
            if state.type_filter.as_deref() == Some(&name) {
                state.type_loading = false;
                state.rebuild_filtered();
                let effects = select_current(state);
                return DispatchResult::changed_with_many(effects);
            }
            DispatchResult::changed()
        }

        Action::TypeFilterDidError { name, error } => {
            if state.type_filter.as_deref() == Some(&name) {
                state.type_loading = false;
            }
            state.message = Some(format!("Type {name} error: {error}"));
            DispatchResult::changed()
        }

        Action::SelectionMove(delta) => {
            let mut index = state.selected_index as i16 + delta;
            if index < 0 {
                index = 0;
            }
            if !state.set_selected_index(index as usize) {
                return DispatchResult::unchanged();
            }
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::DexSelect(index) => {
            if !state.set_selected_index(index) {
                return DispatchResult::unchanged();
            }
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SelectionPage(delta) => {
            let page = list_page_size(state) as i16;
            let mut index = state.selected_index as i16 + delta * page;
            if index < 0 {
                index = 0;
            }
            if !state.set_selected_index(index as usize) {
                return DispatchResult::unchanged();
            }
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SelectionJumpTop => {
            if !state.set_selected_index(0) {
                return DispatchResult::unchanged();
            }
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SelectionJumpBottom => {
            let last = state.filtered_indices.len().saturating_sub(1);
            if !state.set_selected_index(last) {
                return DispatchResult::unchanged();
            }
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SearchStart => {
            state.search.active = true;
            state.search.query.clear();
            state.rebuild_filtered();
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SearchCancel => {
            if !state.search.active && state.search.query.is_empty() {
                return DispatchResult::unchanged();
            }
            state.search.active = false;
            state.search.query.clear();
            state.rebuild_filtered();
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SearchSubmit => {
            state.search.active = false;
            state.rebuild_filtered();
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SearchInput(ch) => {
            state.search.query.push(ch);
            state.rebuild_filtered();
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::SearchBackspace => {
            state.search.query.pop();
            state.rebuild_filtered();
            DispatchResult::changed_with_many(select_current(state))
        }

        Action::PokemonDidLoad(detail) => {
            let name = detail.name.clone();
            state.details.insert(name.clone(), detail);
            state.detail_loading = false;
            state.message = None;
            let effects = detail_follow_up(state, &name);
            if effects.is_empty() {
                DispatchResult::changed()
            } else {
                DispatchResult::changed_with_many(effects)
            }
        }

        Action::PokemonDidError { name, error } => {
            state.detail_loading = false;
            state.message = Some(format!("{name} load error: {error}"));
            DispatchResult::changed()
        }

        Action::PokemonSpeciesDidLoad(species) => {
            let name = species.name.clone();
            state.species.insert(name.clone(), species);
            let effects = evolution_follow_up(state, &name);
            if effects.is_empty() {
                DispatchResult::changed()
            } else {
                DispatchResult::changed_with_many(effects)
            }
        }

        Action::PokemonSpeciesDidError { name, error } => {
            state.message = Some(format!("{name} species error: {error}"));
            DispatchResult::changed()
        }

        Action::EvolutionDidLoad { id, chain } => {
            if let Some(name) = state.detail_name.as_ref() {
                if let Some(index) = chain.stages.iter().position(|stage| stage == name) {
                    state.evolution_selected_index = index;
                }
            }
            state.evolution.insert(id, chain);
            state.evolution_loading = false;
            DispatchResult::changed()
        }

        Action::EvolutionDidError { id: _, error } => {
            state.evolution_loading = false;
            state.message = Some(format!("Evolution error: {error}"));
            DispatchResult::changed()
        }

        Action::EvolutionSelect(index) => {
            let Some(stage_name) = evolution_stage_name(state, index) else {
                return DispatchResult::unchanged();
            };
            if index == state.evolution_selected_index
                && state.detail_name.as_deref() == Some(&stage_name)
            {
                return DispatchResult::unchanged();
            }
            state.evolution_selected_index = index;
            let effects = select_detail(state, &stage_name);
            if effects.is_empty() {
                DispatchResult::changed()
            } else {
                DispatchResult::changed_with_many(effects)
            }
        }

        Action::SpriteDidLoad { name, sprite } => {
            state.sprite_cache.insert(name, sprite);
            state.sprite_loading = false;
            state.reset_sprite_animation();
            DispatchResult::changed()
        }

        Action::SpriteDidError { name, error } => {
            state.sprite_loading = false;
            state.message = Some(format!("Sprite error for {name}: {error}"));
            DispatchResult::changed()
        }

        Action::MoveDetailDidLoad(detail) => {
            state.move_cache.insert(detail.name.clone(), detail);
            DispatchResult::changed()
        }

        Action::MoveDetailDidError { name, error } => {
            state.message = Some(format!("Move {name} error: {error}"));
            DispatchResult::changed()
        }

        Action::AbilityDetailDidLoad(detail) => {
            state.ability_cache.insert(detail.name.clone(), detail);
            DispatchResult::changed()
        }

        Action::AbilityDetailDidError { name, error } => {
            state.message = Some(format!("Ability {name} error: {error}"));
            DispatchResult::changed()
        }

        Action::EncounterDidLoad { name, encounters } => {
            state.encounter_cache.insert(name.clone(), encounters);
            state.encounter_loading = false;
            if state.detail_name.as_deref() == Some(&name) {
                normalize_encounter_filter(state, &name);
                state.selected_encounter_index = 0;
            }
            DispatchResult::changed()
        }

        Action::EncounterDidError { name, error } => {
            state.encounter_loading = false;
            state.message = Some(format!("Encounters {name} error: {error}"));
            DispatchResult::changed()
        }

        Action::TypeMatchupDidLoad { name, matchup } => {
            state.type_matchup_cache.insert(name, matchup);
            state.type_matchup_loading = current_matchup_loading(state);
            DispatchResult::changed()
        }

        Action::TypeMatchupDidError { name, error } => {
            state.type_matchup_loading = current_matchup_loading(state);
            state.message = Some(format!("Type matchup {name} error: {error}"));
            DispatchResult::changed()
        }

        Action::DetailModeToggle => {
            cycle_detail_tab(state, 1)
        }

        Action::DetailTabNext => cycle_detail_tab(state, 1),

        Action::DetailTabPrev => cycle_detail_tab(state, -1),

        Action::DetailNext => {
            if !advance_detail(state, 1) {
                return DispatchResult::unchanged();
            }
            detail_selection_effects(state)
        }

        Action::DetailPrev => {
            if !advance_detail(state, -1) {
                return DispatchResult::unchanged();
            }
            detail_selection_effects(state)
        }

        Action::MoveSelect(index) => {
            if !select_move_index(state, index) {
                return DispatchResult::unchanged();
            }
            detail_selection_effects(state)
        }

        Action::AbilitySelect(index) => {
            if !select_ability_index(state, index) {
                return DispatchResult::unchanged();
            }
            detail_selection_effects(state)
        }

        Action::EncounterSelect(index) => {
            if !select_encounter_index(state, index) {
                return DispatchResult::unchanged();
            }
            detail_selection_effects(state)
        }

        Action::EncounterFilterNext => cycle_encounter_filter(state, 1),

        Action::EncounterFilterPrev => cycle_encounter_filter(state, -1),

        Action::ToggleFavorite => {
            let Some(name) = state.selected_name() else {
                return DispatchResult::unchanged();
            };
            if state.favorites.contains(&name) {
                state.favorites.remove(&name);
            } else {
                state.favorites.insert(name);
            }
            DispatchResult::changed()
        }

        Action::ToggleTeam => {
            let Some(name) = state.selected_name() else {
                return DispatchResult::unchanged();
            };
            if let Some(pos) = state.team.iter().position(|member| member == &name) {
                state.team.remove(pos);
                return DispatchResult::changed();
            }
            if state.team.len() >= 6 {
                state.message = Some("Team is full (6).".to_string());
                return DispatchResult::changed();
            }
            state.team.push(name);
            DispatchResult::changed()
        }

        Action::PlayCry => {
            let Some(detail) = state.current_detail() else {
                return DispatchResult::unchanged();
            };
            let Some(url) = detail.cries_latest.clone().or(detail.cries_legacy.clone()) else {
                state.message = Some("No cry available.".to_string());
                return DispatchResult::changed();
            };
            DispatchResult::changed_with(Effect::PlayCry {
                name: detail.name.clone(),
                url,
            })
        }

        Action::CryDidError(error) => {
            state.message = Some(format!("Cry error: {error}"));
            DispatchResult::changed()
        }

        Action::UiTerminalResize(width, height) => {
            if state.terminal_size != (width, height) {
                state.terminal_size = (width, height);
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }

        Action::Tick => tick_animation(state),

        Action::Quit => DispatchResult::unchanged(),
    }
}

fn cycle_filter(state: &mut AppState, step: i16) -> DispatchResult<Effect> {
    if state.type_list.is_empty() {
        return DispatchResult::unchanged();
    }

    let list_len = state.type_list.len() as i16;
    let current_index = state
        .type_filter
        .as_ref()
        .and_then(|name| state.type_list.iter().position(|t| t == name))
        .map(|idx| idx as i16 + 1)
        .unwrap_or(0);
    let mut next = current_index + step;
    let max_index = list_len;
    if next < 0 {
        next = max_index;
    } else if next > max_index {
        next = 0;
    }

    if next == 0 {
        state.type_filter = None;
        state.type_members.clear();
        state.type_loading = false;
        state.rebuild_filtered();
        let effects = select_current(state);
        return DispatchResult::changed_with_many(effects);
    }

    let next_type = state.type_list[(next - 1) as usize].clone();
    state.type_filter = Some(next_type.clone());
    if let Some(cached) = state.type_cache.get(&next_type).cloned() {
        state.type_members = cached;
        state.type_loading = false;
        state.rebuild_filtered();
        let effects = select_current(state);
        return DispatchResult::changed_with_many(effects);
    }

    state.type_members.clear();
    state.type_loading = true;
    state.rebuild_filtered();
    DispatchResult::changed_with_many(vec![Effect::LoadTypeDetail { name: next_type }])
}

fn cycle_region(state: &mut AppState, step: i16) -> DispatchResult<Effect> {
    if state.regions.is_empty() {
        return DispatchResult::unchanged();
    }
    let len = state.regions.len() as i16;
    let mut next = state.region_index as i16 + step;
    if next < 0 {
        next = len - 1;
    } else if next >= len {
        next = 0;
    }
    let next_index = next as usize;
    if next_index == state.region_index {
        return DispatchResult::unchanged();
    }
    state.region_index = next_index;
    state.search.active = false;
    state.search.query.clear();
    state.detail_name = None;
    state.selected_index = 0;
    state.list_loading = true;
    state.pokedex.clear();
    state.pokedex_all.clear();
    state.filtered_indices.clear();
    state.species_index_loading = true;
    state.evolution_selected_index = 0;
    state.reset_sprite_animation();
    state.reset_detail_selection();
    state.message = None;
    if let Some(region) = state.current_region() {
        return DispatchResult::changed_with(Effect::LoadPokedex {
            name: region.name.clone(),
        });
    }
    DispatchResult::changed()
}

fn select_current(state: &mut AppState) -> Vec<Effect> {
    let Some(name) = state.selected_name() else {
        state.detail_name = None;
        return Vec::new();
    };
    select_detail(state, &name)
}

fn select_detail(state: &mut AppState, name: &str) -> Vec<Effect> {
    if state.detail_name.as_deref() == Some(name) {
        return Vec::new();
    }
    state.detail_name = Some(name.to_string());
    state.seen.insert(name.to_string());
    state.reset_sprite_animation();
    state.reset_detail_selection();
    state.encounter_loading = false;
    state.type_matchup_loading = false;
    sync_evolution_selection(state);
    detail_follow_up(state, name)
}

fn tick_animation(state: &mut AppState) -> DispatchResult<Effect> {
    state.tick = state.tick.wrapping_add(1);
    let Some(name) = state.detail_name.as_ref() else {
        return DispatchResult::unchanged();
    };
    let Some(sprite) = state.sprite_cache.get(name) else {
        return DispatchResult::unchanged();
    };
    if sprite.frames.len() <= 1 {
        return DispatchResult::unchanged();
    }
    const FRAME_STEP: u64 = 1;
    state.sprite_frame_tick = state.sprite_frame_tick.wrapping_add(1);
    if state.sprite_frame_tick % FRAME_STEP == 0 {
        state.sprite_frame_index = (state.sprite_frame_index + 1) % sprite.frames.len();
        return DispatchResult::changed();
    }
    DispatchResult::unchanged()
}

fn detail_follow_up(state: &mut AppState, name: &str) -> Vec<Effect> {
    let mut effects = Vec::new();
    if !state.details.contains_key(name) {
        state.detail_loading = true;
        effects.push(Effect::LoadPokemonDetail {
            name: name.to_string(),
        });
        return effects;
    }

    if !state.species.contains_key(name) {
        effects.push(Effect::LoadPokemonSpecies {
            name: name.to_string(),
        });
    }

    let mut detail_types = None;
    if let Some(detail) = state.details.get(name) {
        if !state.sprite_cache.contains_key(name) {
            if let Some(url) = detail
                .sprite_animated
                .clone()
                .or(detail.sprite_front_default.clone())
            {
                state.sprite_loading = true;
                effects.push(Effect::LoadSprite {
                    name: name.to_string(),
                    url,
                });
            }
        }
        effects.extend(detail_move_effects(state, detail));
        detail_types = Some(detail.types.clone());
    }

    effects.extend(detail_encounter_effects(state, name));
    if let Some(types) = detail_types {
        effects.extend(detail_matchup_effects(state, &types));
    }
    effects.extend(evolution_follow_up(state, name));
    effects
}

fn evolution_stage_name(state: &AppState, index: usize) -> Option<String> {
    let chain = current_evolution_chain(state)?;
    chain.stages.get(index).cloned()
}

fn current_evolution_chain(state: &AppState) -> Option<&crate::state::EvolutionChain> {
    let species = state.current_species()?;
    let url = species.evolution_chain_url.as_ref()?;
    let id = evolution_id_from_url(url);
    state.evolution.get(&id)
}

fn sync_evolution_selection(state: &mut AppState) {
    let Some(name) = state.detail_name.as_ref() else {
        return;
    };
    let Some(chain) = current_evolution_chain(state) else {
        return;
    };
    if let Some(index) = chain.stages.iter().position(|stage| stage == name) {
        state.evolution_selected_index = index;
    }
}

fn detail_selection_effects(state: &mut AppState) -> DispatchResult<Effect> {
    let (detail_name, detail_types, mut effects) = {
        let Some(detail) = state.current_detail() else {
            return DispatchResult::changed();
        };
        (
            detail.name.clone(),
            detail.types.clone(),
            detail_move_effects(state, detail),
        )
    };
    effects.extend(detail_encounter_effects(state, &detail_name));
    effects.extend(detail_matchup_effects(state, &detail_types));
    if effects.is_empty() {
        DispatchResult::changed()
    } else {
        DispatchResult::changed_with_many(effects)
    }
}

fn advance_detail(state: &mut AppState, delta: i16) -> bool {
    let Some(detail) = state.current_detail() else {
        return false;
    };
    match state.detail_mode {
        crate::state::DetailMode::Move => {
            if detail.moves.is_empty() {
                return false;
            }
            let new_index = clamp_index(state.selected_move_index, detail.moves.len(), delta);
            if new_index == state.selected_move_index {
                return false;
            }
            state.selected_move_index = new_index;
        }
        crate::state::DetailMode::Ability => {
            if detail.abilities.is_empty() {
                return false;
            }
            let new_index = clamp_index(state.selected_ability_index, detail.abilities.len(), delta);
            if new_index == state.selected_ability_index {
                return false;
            }
            state.selected_ability_index = new_index;
        }
        crate::state::DetailMode::Encounter => {
            let Some(name) = state.detail_name.as_ref() else {
                return false;
            };
            let Some(encounters) = state.encounter_cache.get(name) else {
                return false;
            };
            if encounters.is_empty() {
                return false;
            }
            let new_index =
                clamp_index(state.selected_encounter_index, encounters.len(), delta);
            if new_index == state.selected_encounter_index {
                return false;
            }
            state.selected_encounter_index = new_index;
        }
        crate::state::DetailMode::Matchup => {
            return false;
        }
        crate::state::DetailMode::General => {
            return false;
        }
    }
    true
}

fn select_move_index(state: &mut AppState, index: usize) -> bool {
    let Some(detail) = state.current_detail() else {
        return false;
    };
    if detail.moves.is_empty() {
        return false;
    }
    let bounded = index.min(detail.moves.len().saturating_sub(1));
    if bounded == state.selected_move_index {
        return false;
    }
    state.selected_move_index = bounded;
    true
}

fn select_ability_index(state: &mut AppState, index: usize) -> bool {
    let Some(detail) = state.current_detail() else {
        return false;
    };
    if detail.abilities.is_empty() {
        return false;
    }
    let bounded = index.min(detail.abilities.len().saturating_sub(1));
    if bounded == state.selected_ability_index {
        return false;
    }
    state.selected_ability_index = bounded;
    true
}

fn select_encounter_index(state: &mut AppState, index: usize) -> bool {
    let Some(name) = state.detail_name.as_ref() else {
        return false;
    };
    let Some(encounters) = state.encounter_cache.get(name) else {
        return false;
    };
    if encounters.is_empty() {
        return false;
    }
    let bounded = index.min(encounters.len().saturating_sub(1));
    if bounded == state.selected_encounter_index {
        return false;
    }
    state.selected_encounter_index = bounded;
    true
}

fn cycle_detail_tab(state: &mut AppState, step: i16) -> DispatchResult<Effect> {
    let tabs = [
        crate::state::DetailMode::General,
        crate::state::DetailMode::Move,
        crate::state::DetailMode::Ability,
        crate::state::DetailMode::Encounter,
        crate::state::DetailMode::Matchup,
    ];
    let current = tabs
        .iter()
        .position(|mode| mode == &state.detail_mode)
        .unwrap_or(0) as i16;
    let len = tabs.len() as i16;
    let mut next = current + step;
    if next < 0 {
        next = len - 1;
    } else if next >= len {
        next = 0;
    }
    let next_mode = tabs[next as usize];
    if next_mode == state.detail_mode {
        return DispatchResult::unchanged();
    }
    state.detail_mode = next_mode;
    detail_selection_effects(state)
}

fn clamp_index(current: usize, len: usize, delta: i16) -> usize {
    if len == 0 {
        return 0;
    }
    let mut next = current as i16 + delta;
    if next < 0 {
        next = 0;
    } else if next >= len as i16 {
        next = len as i16 - 1;
    }
    next as usize
}

fn detail_move_effects(state: &AppState, detail: &crate::state::PokemonDetail) -> Vec<Effect> {
    let mut effects = Vec::new();
    match state.detail_mode {
        crate::state::DetailMode::Move => {
            if let Some(move_name) = detail.moves.get(state.selected_move_index) {
                if !state.move_cache.contains_key(move_name) {
                    effects.push(Effect::LoadMoveDetail {
                        name: move_name.clone(),
                    });
                }
            }
        }
        crate::state::DetailMode::Ability => {
            if let Some(ability_name) = detail.abilities.get(state.selected_ability_index) {
                if !state.ability_cache.contains_key(ability_name) {
                    effects.push(Effect::LoadAbilityDetail {
                        name: ability_name.clone(),
                    });
                }
            }
        }
        crate::state::DetailMode::General
        | crate::state::DetailMode::Encounter
        | crate::state::DetailMode::Matchup => {}
    }
    effects
}

fn detail_encounter_effects(state: &mut AppState, detail_name: &str) -> Vec<Effect> {
    if state.detail_mode != crate::state::DetailMode::Encounter {
        return Vec::new();
    }
    if state.encounter_cache.contains_key(detail_name) {
        return Vec::new();
    }
    state.encounter_loading = true;
    vec![Effect::LoadEncounters {
        name: detail_name.to_string(),
    }]
}

fn detail_matchup_effects(state: &mut AppState, types: &[String]) -> Vec<Effect> {
    if state.detail_mode != crate::state::DetailMode::Matchup {
        state.type_matchup_loading = false;
        return Vec::new();
    }
    let mut effects = Vec::new();
    let mut missing = false;
    for type_name in types {
        if !state.type_matchup_cache.contains_key(type_name) {
            effects.push(Effect::LoadTypeMatchup {
                name: type_name.clone(),
            });
            missing = true;
        }
    }
    state.type_matchup_loading = missing;
    effects
}

fn current_matchup_loading(state: &AppState) -> bool {
    if state.detail_mode != crate::state::DetailMode::Matchup {
        return false;
    }
    let Some(detail) = state.current_detail() else {
        return false;
    };
    detail
        .types
        .iter()
        .any(|type_name| !state.type_matchup_cache.contains_key(type_name))
}

fn cycle_encounter_filter(state: &mut AppState, step: i16) -> DispatchResult<Effect> {
    let Some(name) = state.detail_name.as_ref() else {
        return DispatchResult::unchanged();
    };
    let Some(encounters) = state.encounter_cache.get(name) else {
        return DispatchResult::unchanged();
    };
    let mut versions: Vec<String> = encounters
        .iter()
        .flat_map(|location| location.version_details.iter())
        .map(|version| version.version.clone())
        .collect();
    versions.sort();
    versions.dedup();
    if versions.is_empty() {
        return DispatchResult::unchanged();
    }
    let current_index = state
        .encounter_version_filter
        .as_ref()
        .and_then(|version| versions.iter().position(|item| item == version))
        .map(|idx| idx as i16 + 1)
        .unwrap_or(0);
    let max_index = versions.len() as i16;
    let mut next = current_index + step;
    if next < 0 {
        next = max_index;
    } else if next > max_index {
        next = 0;
    }
    if next == 0 {
        state.encounter_version_filter = None;
    } else {
        state.encounter_version_filter = Some(versions[(next - 1) as usize].clone());
    }
    state.selected_encounter_index = 0;
    DispatchResult::changed()
}

fn normalize_encounter_filter(state: &mut AppState, name: &str) {
    let Some(filter) = state.encounter_version_filter.as_ref() else {
        return;
    };
    let Some(encounters) = state.encounter_cache.get(name) else {
        return;
    };
    let has_version = encounters
        .iter()
        .flat_map(|location| location.version_details.iter())
        .any(|version| &version.version == filter);
    if !has_version {
        state.encounter_version_filter = None;
    }
}

fn evolution_follow_up(state: &mut AppState, name: &str) -> Vec<Effect> {
    let mut effects = Vec::new();
    let Some(species) = state.species.get(name) else {
        return effects;
    };
    let Some(url) = species.evolution_chain_url.clone() else {
        return effects;
    };
    let id = evolution_id_from_url(&url);
    if !state.evolution.contains_key(&id) {
        state.evolution_loading = true;
        effects.push(Effect::LoadEvolutionChain { id, url });
    }
    effects
}

fn evolution_id_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("unknown")
        .to_string()
}

fn list_page_size(state: &AppState) -> usize {
    state.terminal_size.1.saturating_sub(8) as usize
}
