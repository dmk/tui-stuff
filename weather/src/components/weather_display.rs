use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Frame, Rect};
use tui_dispatch::EventKind;
use tui_dispatch_components::{
    StatusBar, StatusBarHint, StatusBarProps, StatusBarSection, StatusBarStyle,
};

use super::{Component, WeatherBody, WeatherBodyProps};
use crate::action::Action;
use crate::state::AppState;

pub const ERROR_ICON: &str = "\u{26a0}\u{fe0f}";
/// Props for WeatherDisplay - read-only view of state
pub struct WeatherDisplayProps<'a> {
    pub state: &'a AppState,
    pub is_focused: bool,
}

/// The main weather display component
#[derive(Default)]
pub struct WeatherDisplay;

impl Component<Action> for WeatherDisplay {
    type Props<'a> = WeatherDisplayProps<'a>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = Action> {
        if !props.is_focused {
            return None;
        }

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Char('r') | KeyCode::F(5) => Some(Action::WeatherFetch),
                KeyCode::Char('/') => Some(Action::SearchOpen),
                KeyCode::Char('u') => Some(Action::UiToggleUnits),
                KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
                _ => None,
            },
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: WeatherDisplayProps<'_>) {
        let chunks = Layout::vertical([
            Constraint::Min(1),    // Main content
            Constraint::Length(1), // Help bar
        ])
        .split(area);

        let mut body = WeatherBody;
        body.render(frame, chunks[0], WeatherBodyProps { state: props.state });

        let mut status_bar = StatusBar::new();
        <StatusBar as Component<Action>>::render(
            &mut status_bar,
            frame,
            chunks[1],
            StatusBarProps {
                left: StatusBarSection::empty(),
                center: StatusBarSection::hints(&[
                    StatusBarHint::new("r", "refresh"),
                    StatusBarHint::new("/", "search"),
                    StatusBarHint::new("u", "units"),
                    StatusBarHint::new("q", "quit"),
                ]),
                right: StatusBarSection::empty(),
                style: StatusBarStyle::default(),
                is_focused: false,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WeatherData;
    use tui_dispatch::testing::*;

    #[test]
    fn test_handle_event_refresh() {
        let mut component = WeatherDisplay;
        let state = AppState::default();
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };

        let actions: Vec<_> = component
            .handle_event(&EventKind::Key(key("r")), props)
            .into_iter()
            .collect();
        actions.assert_count(1);
        actions.assert_first(Action::WeatherFetch);
    }

    #[test]
    fn test_handle_event_quit() {
        let mut component = WeatherDisplay;
        let state = AppState::default();
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: true,
        };

        let actions: Vec<_> = component
            .handle_event(&EventKind::Key(key("q")), props)
            .into_iter()
            .collect();
        actions.assert_first(Action::Quit);
    }

    #[test]
    fn test_handle_event_unfocused_ignores() {
        let mut component = WeatherDisplay;
        let state = AppState::default();
        let props = WeatherDisplayProps {
            state: &state,
            is_focused: false,
        };

        let actions: Vec<_> = component
            .handle_event(&EventKind::Key(key("r")), props)
            .into_iter()
            .collect();
        actions.assert_empty();
    }

    #[test]
    fn test_render_loading() {
        use tui_dispatch::DataResource;

        let mut render = RenderHarness::new(60, 24);
        let mut component = WeatherDisplay;

        let state = AppState {
            weather: DataResource::Loading,
            ..Default::default()
        };

        let output = render.render_to_string_plain(|frame| {
            let props = WeatherDisplayProps {
                state: &state,
                is_focused: true,
            };
            component.render(frame, frame.area(), props);
        });

        assert!(!output.is_empty());
    }

    #[test]
    fn test_render_weather() {
        use tui_dispatch::DataResource;

        let mut render = RenderHarness::new(60, 24);
        let mut component = WeatherDisplay;

        let state = AppState {
            weather: DataResource::Loaded(WeatherData {
                temperature: 22.5,
                weather_code: 0,
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

        assert!(output.contains("Clear sky"));
    }
}
