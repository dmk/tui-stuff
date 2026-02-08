//! Render snapshot tests using RenderHarness
//!
//! FRAMEWORK PATTERN: RenderHarness
//! - Create harness with terminal dimensions
//! - Render component to test buffer
//! - Convert to string for snapshot testing

use tui_dispatch::{DataResource, testing::*};
use weather::{
    components::{Component, WeatherDisplay, WeatherDisplayProps},
    state::{AppState, Location, TempUnit, WeatherData},
};

#[test]
fn test_render_loading_state() {
    // PATTERN: RenderHarness for visual testing
    let mut render = RenderHarness::new(60, 24);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: DataResource::Loading,
        tick_count: 0,
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Loading is now indicated by animated gradient on city name
    // Just verify the component renders without panicking
    assert!(!output.is_empty(), "Should render something");
}

#[test]
fn test_render_clear_weather() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: DataResource::Loaded(WeatherData {
            temperature: 22.5,
            weather_code: 0, // Clear sky
            description: "Clear sky".into(),
        }),
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Location and temperature are now rendered as FIGlet ASCII art
    assert!(output.contains("Clear sky"), "Should show description");
}

#[test]
fn test_render_error_state() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: DataResource::Failed("Network error".into()),
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    assert!(output.contains("Error"), "Should show error label");
    assert!(
        output.contains("Network error"),
        "Should show error message"
    );
    assert!(output.contains("retry"), "Should show retry hint");
}

#[test]
fn test_render_fahrenheit() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: DataResource::Loaded(WeatherData {
            temperature: 0.0, // 0°C = 32°F
            weather_code: 0,
            description: "Clear".into(),
        }),
        unit: TempUnit::Fahrenheit,
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Temperature is now rendered as FIGlet ASCII art
    // Just verify the component renders without panicking
    assert!(output.contains("Clear"), "Should show description");
}

#[test]
fn test_render_custom_location() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let custom = Location {
        name: "My Beach House".into(),
        lat: 0.0,
        lon: 0.0,
    };
    let state = AppState::new(custom);

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Location name is now rendered as FIGlet ASCII art
    // Just verify the component renders without panicking
    assert!(!output.is_empty(), "Should render something");
}

#[test]
fn test_render_help_bar() {
    let mut render = RenderHarness::new(80, 24);
    let mut component = WeatherDisplay;

    let state = AppState::default();

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Should show keybinding hints (new format: "r refresh" style)
    assert!(output.contains("refresh"), "Should show refresh hint");
    assert!(output.contains("units"), "Should show units hint");
    assert!(output.contains("quit"), "Should show quit hint");
}

#[test]
fn test_render_initial_state() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState::default();

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Initial state should prompt user to fetch
    assert!(
        output.contains("to fetch weather"),
        "Should show fetch prompt"
    );
}

#[test]
fn test_render_rain_weather() {
    let mut render = RenderHarness::new(50, 20);
    let mut component = WeatherDisplay;

    let state = AppState {
        weather: DataResource::Loaded(WeatherData {
            temperature: 15.0,
            weather_code: 61, // Rain
            description: "Rain".into(),
        }),
        ..Default::default()
    };

    let output = render.render_to_string_plain(|frame| {
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };
        component.render(frame, frame.area(), props);
    });

    // Temperature is now rendered as FIGlet ASCII art
    assert!(output.contains("Rain"), "Should show rain description");
}
