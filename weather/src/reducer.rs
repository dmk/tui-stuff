//! Reducer - pure function: (state, action) -> DispatchResult

use tui_dispatch::{DataResource, DispatchResult};

use crate::action::Action;
use crate::effect::Effect;
use crate::state::{AppState, LOADING_ANIM_CYCLE_TICKS};

/// The reducer handles all state transitions
pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        // ===== Weather actions =====
        Action::WeatherFetch => {
            if state.weather.is_loaded() {
                state.is_refreshing = true;
            } else {
                state.weather = DataResource::Loading;
            }
            state.tick_count = 0;
            state.loading_anim_ticks_remaining = 0;
            let loc = state.current_location();
            DispatchResult::changed_with(Effect::FetchWeather {
                lat: loc.lat,
                lon: loc.lon,
            })
        }

        Action::WeatherDidLoad(data) => {
            state.weather = DataResource::Loaded(data);
            state.is_refreshing = false;
            state.loading_anim_ticks_remaining = ticks_to_phase_zero(state.tick_count);
            DispatchResult::changed()
        }

        Action::WeatherDidError(msg) => {
            state.weather = DataResource::Failed(msg);
            state.is_refreshing = false;
            state.loading_anim_ticks_remaining = ticks_to_phase_zero(state.tick_count);
            DispatchResult::changed()
        }

        // ===== Search actions =====
        Action::SearchOpen => {
            state.search_mode = true;
            state.search_query.clear();
            state.search_results.clear();
            state.search_error = None;
            state.search_selected = 0;
            DispatchResult::changed()
        }

        Action::SearchClose => {
            state.search_mode = false;
            state.search_query.clear();
            state.search_results.clear();
            state.search_error = None;
            state.search_selected = 0;
            DispatchResult::changed()
        }

        Action::SearchQueryChange(query) => {
            state.search_query = query;
            state.search_selected = 0;
            state.search_error = None;
            DispatchResult::changed_with(Effect::SearchCities {
                query: state.search_query.clone(),
            })
        }

        Action::SearchQuerySubmit(query) => {
            let query = query.trim().to_string();
            state.search_query = query.clone();
            state.search_selected = 0;
            state.search_error = None;
            if query.is_empty() {
                state.search_results.clear();
            }
            DispatchResult::changed_with(Effect::SearchCities { query })
        }

        Action::SearchDidLoad(results) => {
            state.search_results = results;
            state.search_error = None;
            state.search_selected = 0;
            DispatchResult::changed()
        }

        Action::SearchDidError(msg) => {
            state.search_results.clear();
            state.search_error = Some(msg);
            state.search_selected = 0;
            DispatchResult::changed()
        }

        Action::SearchSelect(index) => {
            if index < state.search_results.len() && index != state.search_selected {
                state.search_selected = index;
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }

        Action::SearchConfirm => {
            let Some(location) = state.search_results.get(state.search_selected).cloned() else {
                return DispatchResult::unchanged();
            };

            let (lat, lon) = (location.lat, location.lon);
            state.location = location;
            state.weather = DataResource::Loading;
            state.is_refreshing = false;
            state.search_mode = false;
            state.search_query.clear();
            state.search_results.clear();
            state.search_error = None;
            state.search_selected = 0;
            state.tick_count = 0;
            state.loading_anim_ticks_remaining = 0;
            DispatchResult::changed_with(Effect::FetchWeather { lat, lon })
        }

        // ===== UI actions =====
        Action::UiToggleUnits => {
            state.unit = state.unit.toggle();
            DispatchResult::changed()
        }

        Action::Render => DispatchResult::changed(),

        // ===== Global actions =====
        Action::Tick => {
            let animating = state.loading_anim_active();
            if animating {
                state.tick_count = state.tick_count.wrapping_add(1);
                if state.loading_anim_ticks_remaining > 0 {
                    state.loading_anim_ticks_remaining -= 1;
                }
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }

        Action::Quit => DispatchResult::unchanged(),
    }
}

fn ticks_to_phase_zero(tick_count: u32) -> u32 {
    let cycle = LOADING_ANIM_CYCLE_TICKS.max(1);
    if tick_count == 0 {
        return cycle;
    }
    let remainder = tick_count % cycle;
    if remainder == 0 { 0 } else { cycle - remainder }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WeatherData;

    #[test]
    fn test_weather_fetch_sets_loading() {
        let mut state = AppState::default();
        assert!(state.weather.is_empty());
        state.tick_count = 5;
        state.loading_anim_ticks_remaining = 7;

        let result = reducer(&mut state, Action::WeatherFetch);

        assert!(result.changed);
        assert!(state.weather.is_loading());
        assert_eq!(state.tick_count, 0);
        assert_eq!(state.loading_anim_ticks_remaining, 0);
        assert_eq!(result.effects.len(), 1);
        assert!(matches!(result.effects[0], Effect::FetchWeather { .. }));
    }

    #[test]
    fn test_weather_did_load_clears_loading() {
        let mut state = AppState {
            weather: DataResource::Loading,
            tick_count: 1,
            ..Default::default()
        };

        let weather = WeatherData {
            temperature: 22.5,
            weather_code: 0,
            description: "Clear".into(),
        };

        let result = reducer(&mut state, Action::WeatherDidLoad(weather.clone()));

        assert!(result.changed);
        assert!(state.weather.is_loaded());
        assert_eq!(state.weather.data(), Some(&weather));
        assert_eq!(
            state.loading_anim_ticks_remaining,
            LOADING_ANIM_CYCLE_TICKS - 1
        );
    }

    #[test]
    fn test_toggle_units() {
        let mut state = AppState::default();
        assert_eq!(state.unit, crate::state::TempUnit::Celsius);

        reducer(&mut state, Action::UiToggleUnits);
        assert_eq!(state.unit, crate::state::TempUnit::Fahrenheit);

        reducer(&mut state, Action::UiToggleUnits);
        assert_eq!(state.unit, crate::state::TempUnit::Celsius);
    }

    #[test]
    fn test_tick_rerenders_during_loading_animation() {
        let mut state = AppState::default();

        // Not loading and no remaining animation - no re-render
        let result = reducer(&mut state, Action::Tick);
        assert!(!result.changed);

        // Remaining animation ticks - should re-render
        state.loading_anim_ticks_remaining = 1;
        let result = reducer(&mut state, Action::Tick);
        assert!(result.changed);
        assert_eq!(state.loading_anim_ticks_remaining, 0);

        // Loading - should re-render even without remaining ticks
        state.weather = DataResource::Loading;
        state.loading_anim_ticks_remaining = 0;
        let result = reducer(&mut state, Action::Tick);
        assert!(result.changed);
    }
}
