use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use minesweeper::middleware::MinesweeperMiddleware;
use minesweeper::{
    action::Action,
    custom_board, input,
    reducer::reducer,
    state::{AppState, Difficulty},
    ui,
};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use tui_dispatch::{EventKind, StoreWithMiddleware};

#[test]
fn native_input_reveals_flags_and_keeps_custom_form_input_modal() {
    let area = Rect::new(0, 0, 100, 40);
    let mut custom = custom_board::Ui::default();
    let mut store = StoreWithMiddleware::new(
        AppState::new(Difficulty::Beginner),
        reducer,
        MinesweeperMiddleware::default(),
    );
    for button in [MouseButton::Right, MouseButton::Left] {
        let (column, row, x, y) = (0..area.height)
            .flat_map(|row| (0..area.width).map(move |column| (column, row)))
            .find_map(|(column, row)| {
                ui::cell_at(area, store.state(), column, row).map(|(x, y)| (column, row, x, y))
            })
            .unwrap();
        let event = EventKind::Mouse(MouseEvent {
            kind: MouseEventKind::Down(button),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
        let actions = input::map_event(&event, store.state(), area, &mut custom).actions;
        assert_eq!(actions.len(), 1);
        for action in actions {
            store.dispatch(action);
        }
        if button == MouseButton::Left {
            assert!(
                !store.state().grid[y][x].revealed,
                "left click must respect flags"
            );
        } else {
            assert!(store.state().grid[y][x].flagged);
        }
    }
    for code in [
        KeyCode::Char('c'),
        KeyCode::Char('1'),
        KeyCode::Char('2'),
        KeyCode::Down,
        KeyCode::Char('1'),
        KeyCode::Char('5'),
        KeyCode::Down,
        KeyCode::Char('3'),
        KeyCode::Char('0'),
        KeyCode::Enter,
    ] {
        for action in input::map_event(
            &EventKind::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            store.state(),
            area,
            &mut custom,
        )
        .actions
        {
            store.dispatch(action);
        }
    }
    assert!(store.state().custom_form.is_none());
    assert_eq!(
        (
            store.state().width,
            store.state().height,
            store.state().mine_count
        ),
        (12, 15, 30)
    );
    let release = EventKind::Key(KeyEvent::new_with_kind(
        KeyCode::Char(' '),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert!(input::map_event(&release, store.state(), area, &mut custom)
        .actions
        .is_empty());
    store.dispatch(Action::CustomOpen);
    let escape = EventKind::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(
        input::map_event(&escape, store.state(), area, &mut custom)
            .actions
            .as_slice(),
        [Action::CustomCancel]
    ));
    let interrupt = EventKind::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(matches!(
        input::map_event(&interrupt, store.state(), area, &mut custom)
            .actions
            .as_slice(),
        [Action::Quit]
    ));
}

#[test]
fn native_binary_renders_and_custom_boards_fit_tiny_viewports() {
    for (width, height) in [(1, 1), (2, 2), (20, 8), (60, 22), (100, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut state = AppState::new(Difficulty::Custom {
            width: 120,
            height: 120,
            mines: 4320,
        });
        let mut custom = custom_board::Ui::default();
        for form in [false, true] {
            if form {
                reducer(&mut state, Action::CustomOpen);
            }
            terminal
                .draw(|frame| {
                    ui::render_in(frame, frame.area(), &state);
                    custom.render(frame, frame.area(), &state);
                })
                .unwrap();
        }
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_minesweeper"))
        .args([
            "--debug-render-once",
            "--difficulty",
            "3",
            "--width",
            "80",
            "--height",
            "30",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Minesweeper") && text.contains("Expert") && text.contains("q quit"));
    assert_eq!(text.lines().count(), 30);
    assert!(!text.contains('\u{1b}'));
}
