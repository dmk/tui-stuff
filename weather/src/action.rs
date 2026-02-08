//! Actions demonstrating category inference and async patterns

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Location, WeatherData};

/// Application actions with automatic category inference
#[derive(tui_dispatch::Action, Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[action(infer_categories)]
pub enum Action {
    // ===== Weather category =====
    /// Intent: Request weather data fetch (triggers async task)
    WeatherFetch,

    /// Result: Weather data loaded successfully
    WeatherDidLoad(WeatherData),

    /// Result: Weather fetch failed
    WeatherDidError(String),

    // ===== Search category =====
    /// Open city search overlay
    SearchOpen,

    /// Close search overlay (cancel)
    SearchClose,

    /// Search query text changed
    SearchQueryChange(String),

    /// Submit search query (explicit trigger)
    SearchQuerySubmit(String),

    /// Result: Cities found from geocoding API
    SearchDidLoad(Vec<Location>),

    /// Result: Search failed
    SearchDidError(String),

    /// Select a result in the list (by index)
    SearchSelect(usize),

    /// Confirm selection - switch to selected city
    SearchConfirm,

    // ===== UI category =====
    /// Toggle between Celsius and Fahrenheit
    UiToggleUnits,

    /// Force a re-render (for cursor movement, etc.)
    Render,

    // ===== Uncategorized (global) =====
    /// Periodic tick for loading animation
    Tick,

    /// Exit the application
    Quit,
}
