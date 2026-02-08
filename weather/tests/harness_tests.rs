//! Tests using the new StoreTestHarness and EffectStoreTestHarness
//!
//! These tests demonstrate the integrated testing pattern where
//! store, component, and render testing are combined.

use tui_dispatch::testing::*;
use tui_dispatch::{DataResource, NumericComponentId};
use weather::{
    action::Action,
    components::{Component, WeatherDisplay, WeatherDisplayProps},
    effect::Effect,
    reducer::reducer,
    state::{AppState, TempUnit, WeatherData},
};

/// Helper to create mock weather data
fn mock_weather() -> WeatherData {
    WeatherData {
        temperature: 22.5,
        weather_code: 0,
        description: "Clear sky".into(),
    }
}

/// Helper to create state with weather loaded
fn state_with_weather() -> AppState {
    AppState {
        weather: DataResource::Loaded(mock_weather()),
        ..Default::default()
    }
}

// ============================================================================
// EffectStoreTestHarness Tests
// ============================================================================

#[test]
fn test_weather_fetch_flow_with_harness() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Trigger fetch - should set loading and emit effect
    harness.dispatch_collect(Action::WeatherFetch);
    harness.assert_state(|s| s.weather.is_loading());

    // Verify effect was emitted
    let effects = harness.drain_effects();
    effects.effects_count(1);
    effects.effects_first_matches(|e| matches!(e, Effect::FetchWeather { .. }));

    // Simulate async completion
    harness.complete_action(Action::WeatherDidLoad(mock_weather()));
    let (changed, total) = harness.process_emitted();

    assert_eq!(total, 1, "Should have processed 1 action");
    assert_eq!(changed, 1, "Action should have changed state");

    harness.assert_state(|s| s.weather.is_loaded());
    harness.assert_state(|s| s.weather.data().unwrap().description == "Clear sky");
}

#[test]
fn test_weather_error_flow() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Trigger fetch
    harness.dispatch_collect(Action::WeatherFetch);
    harness.assert_state(|s| s.weather.is_loading());

    // Simulate error
    harness.complete_action(Action::WeatherDidError("Network error".into()));
    harness.process_emitted();

    harness.assert_state(|s| s.weather.is_failed());
    harness.assert_state(|s| s.weather.error() == Some("Network error"));
}

#[test]
fn test_unit_toggle_with_harness() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    harness.assert_state(|s| s.unit == TempUnit::Celsius);

    harness.dispatch_collect(Action::UiToggleUnits);
    harness.assert_state(|s| s.unit == TempUnit::Fahrenheit);

    harness.dispatch_collect(Action::UiToggleUnits);
    harness.assert_state(|s| s.unit == TempUnit::Celsius);
}

#[test]
fn test_dispatch_all() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Dispatch multiple actions at once
    let results = harness.dispatch_all([
        Action::UiToggleUnits,
        Action::UiToggleUnits,
        Action::UiToggleUnits,
    ]);

    // All should have changed state
    assert_eq!(results, vec![true, true, true]);

    // Net result: toggled 3 times = Fahrenheit
    harness.assert_state(|s| s.unit == TempUnit::Fahrenheit);
}

// ============================================================================
// Component + Store Integration Tests
// ============================================================================

#[test]
fn test_keyboard_triggers_fetch() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);
    let mut component = WeatherDisplay;

    // Send 'r' key through component, get actions
    let actions = harness.send_keys::<NumericComponentId, _, _>("r", |state, event| {
        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        component
            .handle_event(&event.kind, props)
            .into_iter()
            .collect::<Vec<_>>()
    });

    // Verify action was returned
    actions.assert_count(1);
    actions.assert_first(Action::WeatherFetch);

    // Now dispatch the action manually and verify state + effects
    harness.dispatch_collect(Action::WeatherFetch);
    harness.assert_state(|s| s.weather.is_loading());

    let effects = harness.drain_effects();
    effects.effects_first_matches(|e| matches!(e, Effect::FetchWeather { .. }));
}

#[test]
fn test_keyboard_toggle_units() {
    let mut harness = EffectStoreTestHarness::new(state_with_weather(), reducer);
    let mut component = WeatherDisplay;

    harness.assert_state(|s| s.unit == TempUnit::Celsius);

    // Send 'u' key through component
    let actions = harness.send_keys::<NumericComponentId, _, _>("u", |state, event| {
        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        component
            .handle_event(&event.kind, props)
            .into_iter()
            .collect::<Vec<_>>()
    });

    // Dispatch the returned action
    for action in actions {
        harness.dispatch_collect(action);
    }

    harness.assert_state(|s| s.unit == TempUnit::Fahrenheit);
}

// ============================================================================
// Render Tests with Harness
// ============================================================================

#[test]
fn test_render_loading_state() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);
    let mut component = WeatherDisplay;

    // Trigger loading
    harness.dispatch_collect(Action::WeatherFetch);

    let output = harness.render_plain(60, 20, |frame, area, state| {
        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        component.render(frame, area, props);
    });

    // Loading state - verify location coordinates are shown
    assert!(
        output.contains("50.45") || output.contains("30.52"),
        "Location coordinates should be visible in output:\n{}",
        output
    );
}

#[test]
fn test_render_weather_data() {
    let mut harness = EffectStoreTestHarness::new(state_with_weather(), reducer);
    let mut component = WeatherDisplay;

    let output = harness.render_plain(60, 20, |frame, area, state| {
        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        component.render(frame, area, props);
    });

    // Should show weather description
    assert!(
        output.contains("Clear sky"),
        "Weather description should be visible in output:\n{}",
        output
    );
}

#[test]
fn test_render_unit_toggle_changes_display() {
    let mut harness = EffectStoreTestHarness::new(state_with_weather(), reducer);
    let mut component = WeatherDisplay;

    // Render in Celsius
    let celsius_output = harness.render_plain(60, 20, |frame, area, state| {
        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        component.render(frame, area, props);
    });

    // Toggle to Fahrenheit
    harness.dispatch_collect(Action::UiToggleUnits);

    // Render in Fahrenheit
    let fahrenheit_output = harness.render_plain(60, 20, |frame, area, state| {
        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        component.render(frame, area, props);
    });

    // Outputs should be different (temperature display changes)
    assert_ne!(
        celsius_output, fahrenheit_output,
        "Celsius and Fahrenheit renders should differ"
    );
}

// ============================================================================
// Effect Assertions Tests
// ============================================================================

#[test]
fn test_effect_assertions() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Initially no effects
    let effects = harness.drain_effects();
    effects.effects_empty();

    // After fetch, should have exactly one effect
    harness.dispatch_collect(Action::WeatherFetch);
    let effects = harness.drain_effects();
    effects.effects_not_empty();
    effects.effects_count(1);
    effects.effects_all_match(|e| matches!(e, Effect::FetchWeather { .. }));
    effects.effects_none_match(|e| matches!(e, Effect::SearchCities { .. }));
}

#[test]
fn test_search_triggers_effect() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Open search and submit query
    harness.dispatch_collect(Action::SearchOpen);
    harness.dispatch_collect(Action::SearchQuerySubmit("London".into()));

    let effects = harness.drain_effects();
    effects.effects_count(1);
    effects.effects_first_matches(
        |e| matches!(e, Effect::SearchCities { query } if query == "London"),
    );
}

// ============================================================================
// Async Simulation Tests
// ============================================================================

#[test]
fn test_multiple_async_completions() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Queue up multiple async completions
    harness.complete_action(Action::WeatherDidLoad(mock_weather()));
    harness.complete_action(Action::UiToggleUnits);

    // Process all at once
    let (changed, total) = harness.process_emitted();

    assert_eq!(total, 2);
    assert_eq!(changed, 2);

    // State should reflect both actions
    harness.assert_state(|s| s.weather.is_loaded());
    harness.assert_state(|s| s.unit == TempUnit::Fahrenheit);
}
