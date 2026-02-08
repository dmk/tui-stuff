use artbox::{
    Alignment as ArtAlignment, Color as ArtColor, Fill, LinearGradient, Renderer, fonts,
    integrations::ratatui::ArtBox,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use tui_dispatch::DataResource;

use super::{Component, ERROR_ICON, LocationHeader, LocationHeaderProps};
use super::location_header::HEADER_OVERHEAD;
use crate::action::Action;
use crate::sprites::{self, SpriteSize};
use crate::state::{AppState, WeatherData};

pub struct WeatherBody;

pub struct WeatherBodyProps<'a> {
    pub state: &'a AppState,
}

/// Fixed rows: blank + blank + description.
const LAYOUT_FIXED: u16 = 3;

/// Text cap tiers: (header_cap, temp_cap).
/// terminus(6), miniwi(4), plain(1) — with HEADER_OVERHEAD added to header.
const TEXT_TIERS: [(u16, u16); 3] = [
    (6 + HEADER_OVERHEAD, 6), // terminus for both
    (4 + HEADER_OVERHEAD, 4), // miniwi for both
    (1 + HEADER_OVERHEAD, 1), // plain for both
];

fn font_stack() -> Vec<artbox::Font> {
    fonts::stack(&["terminus", "miniwi"])
}

struct LayoutSizing {
    sprite: Option<SpriteSize>,
    sprite_h: u16,
    header_cap: u16,
    temp_cap: u16,
}

/// Try to fit the largest sprite by progressively shrinking text caps.
/// Only falls to emoji when no sprite fits even with plain text.
fn compute_layout(area_height: u16) -> LayoutSizing {
    for &(hcap, tcap) in &TEXT_TIERS {
        let budget = area_height.saturating_sub(hcap + tcap + LAYOUT_FIXED);
        if let Some(size) = SpriteSize::for_height(budget) {
            let sprite_h = sprites::get_sprite(sprites::WeatherCondition::ClearSky, size)
                .lines
                .len() as u16;
            return LayoutSizing {
                sprite: Some(size),
                sprite_h,
                header_cap: hcap,
                temp_cap: tcap,
            };
        }
    }
    // No sprite fits — emoji with largest text caps
    let (hcap, tcap) = TEXT_TIERS[0];
    LayoutSizing {
        sprite: None,
        sprite_h: 1,
        header_cap: hcap,
        temp_cap: tcap,
    }
}

// ============================================================================
// Component
// ============================================================================

impl Component<Action> for WeatherBody {
    type Props<'a> = WeatherBodyProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let sizing = compute_layout(area.height);

        let view = WeatherView::from_state(props.state);
        match view {
            WeatherView::Error(error) => render_error(frame, area, error),
            WeatherView::Ready(weather) => {
                render_ready(frame, area, props.state, weather, &sizing);
            }
            WeatherView::Loading => {
                render_placeholder(frame, area, props.state, &sizing, "Loading...");
            }
            WeatherView::Empty => {
                render_placeholder_hint(frame, area, props.state, &sizing);
            }
        }
    }
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut header = LocationHeader;
    header.render(
        frame,
        area,
        LocationHeaderProps {
            location: state.current_location(),
            temperature: state.weather.data().map(|w| w.temperature),
            is_animating: state.loading_anim_active(),
            tick_count: state.tick_count,
        },
    );
}

fn make_layout(area: Rect, sizing: &LayoutSizing) -> std::rc::Rc<[Rect]> {
    Layout::vertical([
        Constraint::Max(sizing.header_cap),
        Constraint::Length(1),
        Constraint::Length(sizing.sprite_h),
        Constraint::Length(1),
        Constraint::Max(sizing.temp_cap),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .split(area)
}

fn render_ready(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    weather: &WeatherData,
    sizing: &LayoutSizing,
) {
    let chunks = make_layout(area, sizing);

    render_header(frame, chunks[0], state);

    // Sprite or emoji
    match sizing.sprite {
        Some(size) => {
            let art = sprites::get_sprite(
                sprites::WeatherCondition::from_code(weather.weather_code),
                size,
            );
            frame.render_widget(
                Paragraph::new(art).alignment(Alignment::Center),
                chunks[2],
            );
        }
        None => {
            let emoji = Line::from(sprites::weather_emoji(weather.weather_code)).centered();
            frame.render_widget(Paragraph::new(emoji), chunks[2]);
        }
    }

    // Temperature
    let temp_text = state.unit.format(weather.temperature);
    let renderer = Renderer::new(font_stack())
        .with_plain_fallback()
        .with_alignment(ArtAlignment::Center)
        .with_fill(temperature_gradient(weather.temperature));
    frame.render_widget(ArtBox::new(&renderer, &temp_text), chunks[4]);

    // Description
    let desc = Line::from(vec![Span::styled(
        weather.description.to_string(),
        Style::default().fg(Color::Gray),
    )])
    .centered();
    frame.render_widget(Paragraph::new(desc), chunks[5]);
}

fn render_placeholder(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    sizing: &LayoutSizing,
    message: &str,
) {
    let chunks = make_layout(area, sizing);
    render_header(frame, chunks[0], state);

    let msg = Line::from(vec![Span::styled(
        message,
        Style::default().fg(Color::DarkGray),
    )])
    .centered();
    frame.render_widget(Paragraph::new(msg), chunks[5]);
}

fn render_placeholder_hint(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    sizing: &LayoutSizing,
) {
    let chunks = make_layout(area, sizing);
    render_header(frame, chunks[0], state);

    let hint = Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::DarkGray)),
        Span::styled("r", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" to fetch weather", Style::default().fg(Color::DarkGray)),
    ])
    .centered();
    frame.render_widget(Paragraph::new(hint), chunks[5]);
}

fn render_error(frame: &mut Frame, area: Rect, error: &str) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // blank
        Constraint::Length(1), // icon
        Constraint::Length(1), // "Error"
        Constraint::Length(1), // message
        Constraint::Length(1), // blank
        Constraint::Length(1), // hint
    ])
    .flex(Flex::Center)
    .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(ERROR_ICON).centered()),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(
            Line::from(vec![Span::styled(
                "Error",
                Style::default().fg(Color::Red).bold(),
            )])
            .centered(),
        ),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(
            Line::from(vec![Span::styled(
                error.to_string(),
                Style::default().fg(Color::Rgb(200, 100, 100)),
            )])
            .centered(),
        ),
        chunks[3],
    );
    frame.render_widget(
        Paragraph::new(
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(Color::DarkGray)),
                Span::styled("r", Style::default().fg(Color::Cyan).bold()),
                Span::styled(" to retry", Style::default().fg(Color::DarkGray)),
            ])
            .centered(),
        ),
        chunks[5],
    );
}

// ============================================================================
// Helpers
// ============================================================================

enum WeatherView<'a> {
    Error(&'a str),
    Ready(&'a WeatherData),
    Loading,
    Empty,
}

impl<'a> WeatherView<'a> {
    fn from_state(state: &'a AppState) -> Self {
        match &state.weather {
            DataResource::Failed(error) => WeatherView::Error(error),
            DataResource::Loaded(weather) => WeatherView::Ready(weather),
            DataResource::Loading => WeatherView::Loading,
            DataResource::Empty => WeatherView::Empty,
        }
    }
}

fn temperature_gradient(celsius: f32) -> Fill {
    let (start, end) = match celsius {
        t if t < 0.0 => (
            ArtColor::rgb(150, 200, 255),
            ArtColor::rgb(200, 230, 255),
        ),
        t if t < 15.0 => (
            ArtColor::rgb(100, 180, 255),
            ArtColor::rgb(150, 220, 200),
        ),
        t if t < 25.0 => (
            ArtColor::rgb(100, 200, 150),
            ArtColor::rgb(255, 220, 100),
        ),
        t if t < 35.0 => (
            ArtColor::rgb(255, 180, 80),
            ArtColor::rgb(255, 120, 80),
        ),
        _ => (
            ArtColor::rgb(255, 100, 80),
            ArtColor::rgb(255, 60, 60),
        ),
    };
    Fill::Linear(LinearGradient::horizontal(start, end))
}
