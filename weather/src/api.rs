//! Open-Meteo API client

use serde::Deserialize;

use crate::state::{Location, WeatherData};

// ============================================================================
// Geocoding API
// ============================================================================

/// Geocoding API response from Open-Meteo
#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
}

/// Geocoding error type
#[derive(Debug)]
pub enum GeocodingError {
    Request(reqwest::Error),
    NotFound(String),
}

impl std::fmt::Display for GeocodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeocodingError::Request(e) => write!(f, "Geocoding request failed: {}", e),
            GeocodingError::NotFound(city) => write!(f, "City not found: {}", city),
        }
    }
}

impl std::error::Error for GeocodingError {}

fn location_from_result(result: GeocodingResult) -> Location {
    let display_name = match &result.country {
        Some(country) => format!("{}, {}", result.name, country),
        None => result.name,
    };
    Location {
        name: display_name,
        lat: result.latitude,
        lon: result.longitude,
    }
}

/// Resolve city name to coordinates using Open-Meteo Geocoding API
pub async fn geocode_city(city: &str) -> Result<Location, GeocodingError> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en",
        urlencoding::encode(city)
    );

    let response = reqwest::get(&url).await.map_err(GeocodingError::Request)?;

    let data: GeocodingResponse = response.json().await.map_err(GeocodingError::Request)?;

    data.results
        .and_then(|results| results.into_iter().next())
        .map(location_from_result)
        .ok_or_else(|| GeocodingError::NotFound(city.to_string()))
}

/// Search for cities matching a query using Open-Meteo Geocoding API
pub async fn search_cities(query: &str) -> Result<Vec<Location>, GeocodingError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&language=en",
        urlencoding::encode(query)
    );

    let response = reqwest::get(&url).await.map_err(GeocodingError::Request)?;
    let data: GeocodingResponse = response.json().await.map_err(GeocodingError::Request)?;

    let results = data
        .results
        .unwrap_or_default()
        .into_iter()
        .map(location_from_result)
        .collect();

    Ok(results)
}

// ============================================================================
// Weather API
// ============================================================================

/// API response from Open-Meteo
#[derive(Debug, Deserialize)]
struct WeatherResponse {
    current_weather: CurrentWeather,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature: f32,
    weathercode: u8,
}

/// Fetch weather data from Open-Meteo API
pub async fn fetch_weather_data(lat: f64, lon: f64) -> Result<WeatherData, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
        lat, lon
    );

    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let data: WeatherResponse = response.json().await.map_err(|e| e.to_string())?;

    Ok(WeatherData {
        temperature: data.current_weather.temperature,
        weather_code: data.current_weather.weathercode,
        description: weather_description(data.current_weather.weathercode),
    })
}

/// Convert WMO weather code to human-readable description
fn weather_description(code: u8) -> String {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 => "Snow",
        77 => "Snow grains",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
    .to_string()
}
