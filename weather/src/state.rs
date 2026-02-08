//! Application state - single source of truth

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tui_dispatch::DataResource;

/// Weather data from Open-Meteo API
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeatherData {
    pub temperature: f32,
    pub weather_code: u8, // WMO weather code
    pub description: String,
}

/// A geographic location
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Location {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

/// Temperature unit preference
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub enum TempUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl TempUnit {
    pub fn toggle(&self) -> Self {
        match self {
            TempUnit::Celsius => TempUnit::Fahrenheit,
            TempUnit::Fahrenheit => TempUnit::Celsius,
        }
    }

    pub fn format(&self, celsius: f32) -> String {
        match self {
            TempUnit::Celsius => format!("{:.1}°C", celsius),
            TempUnit::Fahrenheit => format!("{:.1}°F", celsius * 9.0 / 5.0 + 32.0),
        }
    }
}

/// Animation timing for the header gradient seam.
pub const LOADING_ANIM_TICK_MS: u64 = 15;
pub const LOADING_ANIM_CYCLE_TICKS: u32 = 60;

/// Application state - everything the UI needs to render
#[derive(Clone, Debug, tui_dispatch::DebugState, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AppState {
    // --- Core data (visible in debug) ---
    /// Single location (from geocoding)
    #[debug(section = "Location", label = "City", debug_fmt)]
    pub location: Location,

    /// Weather data lifecycle: Empty → Loading → Loaded/Failed
    #[debug(section = "Weather", label = "Data", debug_fmt)]
    pub weather: DataResource<WeatherData>,

    /// Whether a refresh is in progress (keeps showing current data during fetch)
    #[debug(section = "Weather", label = "Refreshing")]
    pub is_refreshing: bool,

    /// Temperature unit preference
    #[debug(section = "Weather", label = "Unit", debug_fmt)]
    pub unit: TempUnit,

    // --- Animation internals (skipped) ---
    /// Animation frame counter (for gradient seam)
    #[debug(skip)]
    pub tick_count: u32,

    /// Remaining ticks to finish the current animation cycle after loading
    #[debug(skip)]
    pub loading_anim_ticks_remaining: u32,

    // --- Search mode (skipped) ---
    /// Whether search overlay is open
    #[debug(skip)]
    pub search_mode: bool,

    /// Current search query
    #[debug(skip)]
    pub search_query: String,

    /// Search results from geocoding API
    #[debug(skip)]
    pub search_results: Vec<Location>,

    /// Search error message
    #[debug(skip)]
    pub search_error: Option<String>,

    /// Selected index in search results
    #[debug(skip)]
    pub search_selected: usize,
}

impl AppState {
    /// Create state with the given location
    pub fn new(location: Location) -> Self {
        Self {
            location,
            weather: DataResource::Empty,
            is_refreshing: false,
            unit: TempUnit::default(),
            tick_count: 0,
            loading_anim_ticks_remaining: 0,
            search_mode: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_error: None,
            search_selected: 0,
        }
    }

    /// Get current location
    pub fn current_location(&self) -> &Location {
        &self.location
    }

    pub fn loading_anim_active(&self) -> bool {
        self.weather.is_loading() || self.is_refreshing || self.loading_anim_ticks_remaining > 0
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(Location {
            name: "Kyiv, Ukraine".into(),
            lat: 50.4501,
            lon: 30.5234,
        })
    }
}
