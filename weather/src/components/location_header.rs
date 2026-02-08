use artbox::{
    Alignment as ArtAlignment, Color as ArtColor, Fill, LinearGradient, Renderer, fonts,
    integrations::ratatui::ArtBox,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::cmp::Ordering;

use super::Component;
use crate::action::Action;
use crate::state::{LOADING_ANIM_CYCLE_TICKS, Location};

pub struct LocationHeader;

pub struct LocationHeaderProps<'a> {
    pub location: &'a Location,
    pub temperature: Option<f32>,
    pub is_animating: bool,
    pub tick_count: u32,
}

/// Overhead inside the header area: 1 spacer + 1 coords line.
/// The FIGlet city name gets `area.height - HEADER_OVERHEAD`.
pub const HEADER_OVERHEAD: u16 = 2;

fn gradient_colors(temp: Option<f32>) -> (ArtColor, ArtColor) {
    match temp {
        Some(t) if t < 0.0 => (
            ArtColor::rgb(150, 200, 255), // Ice blue
            ArtColor::rgb(200, 230, 255), // Light ice
        ),
        Some(t) if t < 15.0 => (
            ArtColor::rgb(100, 180, 255), // Cool blue
            ArtColor::rgb(150, 220, 200), // Teal
        ),
        Some(t) if t < 25.0 => (
            ArtColor::rgb(100, 200, 150), // Green
            ArtColor::rgb(255, 220, 100), // Yellow
        ),
        Some(t) if t < 35.0 => (
            ArtColor::rgb(255, 180, 80), // Orange
            ArtColor::rgb(255, 120, 80), // Deep orange
        ),
        Some(_) => (
            ArtColor::rgb(255, 100, 80), // Red-orange
            ArtColor::rgb(255, 60, 60),  // Hot red
        ),
        None => (
            ArtColor::rgb(180, 180, 180), // Gray (no data)
            ArtColor::rgb(220, 220, 220),
        ),
    }
}

fn make_gradient(colors: (ArtColor, ArtColor), angle: f32, phase: f32) -> Fill {
    let phase = phase.rem_euclid(1.0);
    let mid = colors.0.interpolate(colors.1, 0.5);
    let edge = colors.0.interpolate(colors.1, 0.08);
    let base_stops = [
        (0.0, edge),
        (0.35, colors.0),
        (0.5, mid),
        (0.65, colors.1),
        (1.0, edge),
    ];

    let edge_color = sample_color(&base_stops, (1.0 - phase).rem_euclid(1.0));
    let mut shifted = Vec::with_capacity(base_stops.len() + 2);
    shifted.push((0.0, edge_color));
    shifted.push((1.0, edge_color));
    for (pos, color) in base_stops {
        shifted.push(((pos + phase) % 1.0, color));
    }
    shifted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    let stops = shifted
        .into_iter()
        .map(|(pos, color)| artbox::ColorStop::new(pos, color))
        .collect();

    Fill::Linear(LinearGradient::new(angle, stops))
}

fn animated_phase(tick_count: u32) -> f32 {
    let steps = LOADING_ANIM_CYCLE_TICKS.max(1);
    (tick_count % steps) as f32 / steps as f32
}

fn sample_color(stops: &[(f32, ArtColor)], position: f32) -> ArtColor {
    let pos = position.clamp(0.0, 1.0);
    let mut prev = stops[0];
    for stop in stops.iter() {
        if stop.0 >= pos {
            if (stop.0 - prev.0).abs() < f32::EPSILON {
                return stop.1;
            }
            let t = (pos - prev.0) / (stop.0 - prev.0);
            return prev.1.interpolate(stop.1, t);
        }
        prev = *stop;
    }
    stops.last().unwrap().1
}

impl Component<Action> for LocationHeader {
    type Props<'a> = LocationHeaderProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let chunks = Layout::vertical([
            Constraint::Fill(1),   // FIGlet city name — artbox picks the best font
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Coordinates
        ])
        .split(area);

        let colors = gradient_colors(props.temperature);
        let angle = 5.0;
        let phase = if props.is_animating {
            animated_phase(props.tick_count)
        } else {
            0.0
        };
        let fill = make_gradient(colors, angle, phase);

        let renderer = Renderer::new(fonts::stack(&["terminus", "miniwi"]))
            .with_plain_fallback()
            .with_alignment(ArtAlignment::Center)
            .with_fill(fill);

        let city_widget = ArtBox::new(&renderer, &props.location.name);
        frame.render_widget(city_widget, chunks[0]);

        let coords_line = Line::from(vec![Span::styled(
            format!("{:.2}°N, {:.2}°E", props.location.lat, props.location.lon),
            Style::default().fg(Color::DarkGray),
        )])
        .centered();
        frame.render_widget(Paragraph::new(coords_line), chunks[2]);
    }
}
