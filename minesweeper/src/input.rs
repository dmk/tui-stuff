use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use tui_dispatch::{EventKind, EventOutcome};

use crate::{
    action::Action,
    custom_board,
    state::{AppState, Difficulty},
    ui,
};

pub fn map_event(
    event: &EventKind,
    state: &AppState,
    area: Rect,
    custom: &mut custom_board::Ui,
) -> EventOutcome<Action> {
    if let EventKind::Key(key) = event {
        if key.kind == KeyEventKind::Release {
            return EventOutcome::default();
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return EventOutcome::action(Action::Quit);
        }
    }
    if let Some(form) = &state.custom_form {
        return match event {
            EventKind::Key(_) => EventOutcome::from_actions(custom.key(event, state)),
            EventKind::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                EventOutcome::from_actions(custom_board::click(
                    area,
                    mouse.column,
                    mouse.row,
                    form.field,
                ))
            }
            EventKind::Resize(..) => EventOutcome::default().with_render(),
            _ => EventOutcome::default(),
        };
    }
    let action = match event {
        EventKind::Key(key) => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
            KeyCode::Char(' ') | KeyCode::Enter => {
                Some(Action::Reveal(state.cursor_x, state.cursor_y))
            }
            KeyCode::Char('f') => Some(Action::ToggleFlag(state.cursor_x, state.cursor_y)),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::CursorUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::CursorDown),
            KeyCode::Left | KeyCode::Char('h') => Some(Action::CursorLeft),
            KeyCode::Right | KeyCode::Char('l') => Some(Action::CursorRight),
            KeyCode::Char('1') => Some(Action::SetDifficulty(Difficulty::Beginner)),
            KeyCode::Char('2') => Some(Action::SetDifficulty(Difficulty::Intermediate)),
            KeyCode::Char('3') => Some(Action::SetDifficulty(Difficulty::Expert)),
            KeyCode::Char('n') => Some(Action::NewGame),
            KeyCode::Char('c') => {
                *custom = custom_board::Ui::default();
                Some(Action::CustomOpen)
            }
            _ => None,
        },
        EventKind::Mouse(mouse) => match mouse.kind {
            MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Right)) => {
                ui::cell_at(area, state, mouse.column, mouse.row).map(|(x, y)| {
                    if button == MouseButton::Right {
                        Action::ToggleFlag(x, y)
                    } else {
                        Action::Reveal(x, y)
                    }
                })
            }
            _ => None,
        },
        EventKind::Resize(..) => return EventOutcome::default().with_render(),
        _ => None,
    };
    EventOutcome::from_actions(action)
}
