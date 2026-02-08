pub mod location_header;
pub mod search_overlay;
pub mod weather_body;
pub mod weather_display;

// Re-export core Component trait
pub use tui_dispatch::Component;

pub use location_header::{LocationHeader, LocationHeaderProps};
pub use search_overlay::{SearchOverlay, SearchOverlayProps};
pub use weather_body::{WeatherBody, WeatherBodyProps};
pub use weather_display::{ERROR_ICON, WeatherDisplay, WeatherDisplayProps};
