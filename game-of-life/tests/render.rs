use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use game_of_life::{
    action::Action,
    reducer::reducer,
    state::{AppState, WORLD_WIDTH},
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
                assert!(text.contains("Game of Life"));
                assert!(text.contains("MIDNIGHT GARDEN"));
                assert!(!text.contains("PATTERN LIBRARY"));
                assert!(!text.contains('╭'));
                assert!(text.contains("space run/pause"));
                assert!(text.contains("population"));
                state.show_help = true;
                let help =
                    harness.render_to_string_plain(|frame| ui::render(frame, frame.area(), &state));
                assert!(help.contains("Patterns"));
                assert!(help.contains("Esc / ?") && help.contains("Acorn"));
                assert!(help.contains("Toggle cell") && help.contains("Recenter"));
            } else if width >= 40 {
                assert!(text.contains("Simulation suspended"));
            }
        }
    }
}

#[test]
fn largest_zoom_draws_square_half_blocks_with_equal_gutters() {
    let mut state = AppState {
        zoom: 2,
        show_grid: false,
        ..AppState::default()
    };
    state.cells.fill(0);
    state.trails.fill(0);
    let view = BoardView::for_state(&state);
    let (col, row) = (view.cols / 2, view.rows / 2);
    let (x, y) = (view.origin_x + col, view.origin_y + row);
    state.cells[(y * WORLD_WIDTH + x) as usize] = 1;
    let px = view.area.x + col as u16 * view.cell_width;
    let py = view.area.y + row as u16 * view.cell_height;
    let mut harness = RenderHarness::new(120, 42);
    let buffer = harness.render(|frame| ui::render(frame, frame.area(), &state));
    let (bright, base) = ui::age_colors(1);
    for dx in 0..3 {
        let top = &buffer[(px + dx, py)];
        let bottom = &buffer[(px + dx, py + 1)];
        assert_eq!(top.symbol(), "▀");
        assert_eq!(top.fg, bright);
        assert_ne!(top.bg, ui::FIELD);
        assert_eq!(bottom.symbol(), "▀");
        assert_eq!(bottom.fg, base);
        assert_eq!(bottom.bg, ui::FIELD, "bottom half is the vertical gutter");
    }
    for dy in 0..2 {
        assert_eq!(buffer[(px + 3, py + dy)].symbol(), " ");
        assert_eq!(buffer[(px + 3, py + dy)].bg, ui::FIELD);
        assert_eq!(view.world_at(px + 3, py + dy), Some((x, y)));
    }
}

#[test]
fn smallest_zoom_packs_independent_cells_into_both_halves_of_one_character() {
    let mut state = AppState {
        zoom: 0,
        show_grid: false,
        ..AppState::default()
    };
    state.cells.fill(0);
    state.trails.fill(0);
    let view = BoardView::for_state(&state);
    assert_eq!(view.rows, i32::from(view.area.height) * 2);
    let (x, y) = (view.origin_x + 3, view.origin_y + 4);
    let index = |x, y| (y * WORLD_WIDTH + x) as usize;
    state.cells[index(x, y)] = 1;
    state.cells[index(x + 1, y + 1)] = 40;
    state.cells[index(x + 2, y)] = 1;
    state.cells[index(x + 2, y + 1)] = 40;
    state.trails[index(x + 3, y + 1)] = 8;
    state.cursor_visible = true;
    state.cursor_x = x + 4;
    state.cursor_y = y + 1;
    let mut harness = RenderHarness::new(120, 42);
    let buffer = harness.render(|frame| ui::render(frame, frame.area(), &state));
    let (px, py) = (view.area.x + 3, view.area.y + 2);
    assert_eq!(buffer[(px, py)].symbol(), "▀");
    assert_eq!(buffer[(px, py)].fg, ui::age_colors(1).0);
    assert_eq!(buffer[(px, py)].bg, ui::FIELD);
    assert_eq!(buffer[(px + 1, py)].symbol(), "▄");
    assert_eq!(buffer[(px + 1, py)].fg, ui::age_colors(40).0);
    assert_eq!(buffer[(px + 1, py)].bg, ui::FIELD);
    assert_eq!(buffer[(px + 2, py)].symbol(), "▀");
    assert_eq!(buffer[(px + 2, py)].fg, ui::age_colors(1).0);
    assert_eq!(buffer[(px + 2, py)].bg, ui::age_colors(40).0);
    assert_eq!(
        buffer[(px + 3, py)].symbol(),
        "▄",
        "trails use half-height too"
    );
    assert_eq!(
        buffer[(px + 4, py)].symbol(),
        "▄",
        "cursor identifies the lower cell"
    );
    assert_eq!(view.world_at(px, py), Some((x, y)));
}

#[test]
fn population_chart_shows_recent_samples_and_fits_above_status() {
    let mut state = AppState::default();
    let layout = ui::ScreenLayout::new(ratatui::layout::Rect::new(0, 0, 120, 42));
    let chart = layout.population;
    assert!(layout.field.bottom() < chart.y);
    assert!(chart.bottom() < layout.panel.bottom() - 5);
    state.history = vec![100; 80];
    state.history[20..].fill(0);
    let mut harness = RenderHarness::new(120, 42);
    let buffer = harness.render(|frame| ui::render(frame, frame.area(), &state));
    for y in chart.y..chart.bottom() {
        for x in chart.x + 12..chart.right() {
            assert_eq!(buffer[(x, y)].symbol(), " ", "old samples must scroll out");
        }
    }
    state.history[72..].fill(100);
    let buffer = harness.render(|frame| ui::render(frame, frame.area(), &state));
    assert_eq!(buffer[(chart.right() - 1, chart.y)].symbol(), "█");
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
                y: view.origin_y + 4 * i32::from(view.rows_per_line),
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
