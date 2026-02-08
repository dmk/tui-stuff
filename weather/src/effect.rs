//! Effects - side effects declared by the reducer

/// Side effects that can be triggered by actions
#[derive(Debug, Clone)]
pub enum Effect {
    /// Fetch weather data for the given coordinates
    FetchWeather { lat: f64, lon: f64 },
    /// Search for cities matching the query
    SearchCities { query: String },
}
