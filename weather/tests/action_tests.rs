//! Action and state tests using TestHarness
//!
//! FRAMEWORK PATTERN: TestHarness
//! - Create harness with initial state
//! - Emit actions to simulate user/async events
//! - Drain and assert emitted actions
//! - Use fluent assertions for readable tests

use tui_dispatch::testing::*;
use tui_dispatch::{EffectStore, NumericComponentId, assert_emitted, assert_not_emitted};
use weather::{
    action::Action,
    components::{Component, WeatherDisplay, WeatherDisplayProps},
    effect::Effect,
    reducer::reducer,
    state::{AppState, Location, TempUnit, WeatherData},
};

#[test]
fn test_reducer_weather_fetch() {
    // PATTERN: Create store with reducer, dispatch actions, verify state
    let mut store = EffectStore::new(AppState::default(), reducer);

    // Initial state
    assert!(store.state().weather.is_empty());

    // Dispatch fetch - should set loading and return FetchWeather effect
    let result = store.dispatch(Action::WeatherFetch);
    assert!(result.changed, "State should change");
    assert!(store.state().weather.is_loading());
    assert_eq!(result.effects.len(), 1);
    assert!(matches!(result.effects[0], Effect::FetchWeather { .. }));
}

#[test]
fn test_reducer_weather_load() {
    let mut store = EffectStore::new(AppState::default(), reducer);

    // Simulate fetch completing
    let weather = WeatherData {
        temperature: 22.5,
        weather_code: 0,
        description: "Clear sky".into(),
    };

    store.dispatch(Action::WeatherFetch); // Set loading
    store.dispatch(Action::WeatherDidLoad(weather.clone()));

    assert!(store.state().weather.is_loaded());
    assert_eq!(store.state().weather.data(), Some(&weather));
}

#[test]
fn test_reducer_toggle_units() {
    let mut store = EffectStore::new(AppState::default(), reducer);

    assert_eq!(store.state().unit, TempUnit::Celsius);
    store.dispatch(Action::UiToggleUnits);
    assert_eq!(store.state().unit, TempUnit::Fahrenheit);
    store.dispatch(Action::UiToggleUnits);
    assert_eq!(store.state().unit, TempUnit::Celsius);
}

#[test]
fn test_component_keyboard_events() {
    // PATTERN: TestHarness for component testing
    let mut harness = TestHarness::<AppState, Action>::default();
    let mut component = WeatherDisplay;

    // PATTERN: send_keys helper - parse key strings, call handler
    // NumericComponentId is a simple built-in ComponentId type
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

    // PATTERN: Fluent assertions
    actions.assert_count(1);
    actions.assert_first(Action::WeatherFetch);
}

#[test]
fn test_component_ignores_when_unfocused() {
    let mut harness = TestHarness::<AppState, Action>::default();
    let mut component = WeatherDisplay;

    // When not focused, events should be ignored
    let actions = harness.send_keys::<NumericComponentId, _, _>("r q u", |state, event| {
        let props = WeatherDisplayProps {
            state,
            is_focused: false, // Not focused!
        };
        component
            .handle_event(&event.kind, props)
            .into_iter()
            .collect::<Vec<_>>()
    });

    actions.assert_empty();
}

#[test]
fn test_action_categories() {
    // PATTERN: Category is accessible via the ActionCategory trait
    let did_load = Action::WeatherDidLoad(WeatherData::default());
    let toggle = Action::UiToggleUnits;
    let tick = Action::Tick;

    // Categories are inferred from naming convention
    assert_eq!(did_load.category(), Some("weather_did"));
    assert_eq!(toggle.category(), Some("ui"));
    assert_eq!(tick.category(), None); // Uncategorized

    // Generated predicates for categorized actions
    assert!(did_load.is_weather_did());
    assert!(toggle.is_ui());
}

#[test]
fn test_harness_emit_and_drain() {
    // PATTERN: Emit actions and drain them
    let mut harness = TestHarness::<(), Action>::new(());

    harness.emit(Action::WeatherFetch);
    harness.emit(Action::UiToggleUnits);
    harness.emit(Action::WeatherDidError("oops".into()));

    // Drain all emitted actions
    let actions = harness.drain_emitted();
    actions.assert_count(3);
}

#[test]
fn test_assert_emitted_macro() {
    let actions = vec![
        Action::WeatherFetch,
        Action::WeatherDidLoad(WeatherData::default()),
    ];

    // PATTERN: assert_emitted! macro for pattern matching
    assert_emitted!(actions, Action::WeatherFetch);
    assert_emitted!(actions, Action::WeatherDidLoad(_));
    assert_not_emitted!(actions, Action::Quit);
    assert_not_emitted!(actions, Action::WeatherDidError(_));
}

#[test]
fn test_custom_location() {
    let custom = Location {
        name: "My Place".into(),
        lat: 12.34,
        lon: 56.78,
    };

    let state = AppState::new(custom.clone());

    assert_eq!(state.current_location().name, "My Place");
    assert_eq!(state.current_location().lat, 12.34);
    assert_eq!(state.current_location().lon, 56.78);
}

#[test]
fn test_temp_unit_formatting() {
    // 0°C = 32°F
    assert_eq!(TempUnit::Celsius.format(0.0), "0.0°C");
    assert_eq!(TempUnit::Fahrenheit.format(0.0), "32.0°F");

    // 100°C = 212°F
    assert_eq!(TempUnit::Celsius.format(100.0), "100.0°C");
    assert_eq!(TempUnit::Fahrenheit.format(100.0), "212.0°F");
}
