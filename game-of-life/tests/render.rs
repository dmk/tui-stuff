use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use game_of_life::{
    action::Action,
    reducer::reducer,
    state::AppState,
    ui::{self, BoardView},
};
use ratatui::style::Color;
use tui_dispatch::{testing::RenderHarness, EventKind};

#[test]
fn responsive_frames_and_help_fit_every_supported_zoom() {
    for (width, height) in [
        (1, 1),
        (40, 10),
        (60, 22),
        (80, 24),
        (104, 36),
        (120, 42),
        (176, 60),
    ] {
        for zoom in 0..3 {
            let mut state = AppState {
                viewport_width: width,
                viewport_height: height,
                zoom,
                ..AppState::default()
            };
            let mut harness = RenderHarness::new(width, height);
            let text =
                harness.render_to_string_plain(|frame| ui::render(frame, frame.area(), &state));
            if width >= 60 && height >= 22 {
                assert!(text.contains("L I F E"));
                assert!(text.contains("MIDNIGHT GARDEN"));
                assert_eq!(text.contains("PATTERN LIBRARY"), width >= 105);
                state.show_help = true;
                let help =
                    harness.render_to_string_plain(|frame| ui::render(frame, frame.area(), &state));
                assert!(help.contains("FIELD NOTES"));
                assert!(help.contains("Esc / ?"));
            } else if width >= 40 {
                assert!(text.contains("Simulation suspended"));
            }
        }
    }
}

#[test]
fn actual_buffer_contains_age_colors_and_trails() {
    let mut state = AppState::default();
    reducer(&mut state, Action::Advance(18));
    let mut harness = RenderHarness::new(120, 42);
    let buffer = harness.render(|frame| ui::render(frame, frame.area(), &state));
    assert!(buffer
        .content
        .iter()
        .any(|cell| cell.symbol() == "▀" && cell.fg == ui::age_colors(1).0));
    assert!(buffer
        .content
        .iter()
        .any(|cell| cell.symbol() == "▀" && cell.fg == ui::age_colors(19).0));
    assert!(buffer
        .content
        .iter()
        .any(|cell| cell.symbol() == "░" && matches!(cell.fg, Color::Rgb(_, _, _))));
}

#[test]
fn mouse_paint_coordinates_follow_pan_zoom_and_keyboard_help_blocks_edits() {
    let mut state = AppState::default();
    for zoom in 0..3 {
        state.zoom = zoom;
        state.camera_x = 35;
        let view = BoardView::for_state(&state);
        let column = view.area.x + view.cell_width * 3;
        let row = view.area.y + view.cell_height * 4;
        let event = EventKind::Mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
        let mapped = ui::map_event(&event, &state);
        assert_eq!(
            mapped.actions,
            [Action::CellSet {
                x: view.origin_x + 3,
                y: view.origin_y + 4,
                alive: true
            }]
        );
        reducer(&mut state, mapped.actions[0].clone());
        assert!(!state.running);
        assert_eq!(state.age(state.cursor_x, state.cursor_y), 1);
    }
    state.show_help = true;
    let key = EventKind::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
    assert!(ui::map_event(&key, &state).actions.is_empty());
    let close = EventKind::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(ui::map_event(&close, &state).actions, [Action::HelpToggle]);
}
