use crate::action::Action;
use crate::state::{AppState, CustomForm, GameState};

/// Reducer trusts middleware already validated guards (game over, revealed, flagged).
pub fn reducer(state: &mut AppState, action: Action) -> tui_dispatch::ReducerResult {
    tui_dispatch::ReducerResult {
        changed: reduce(state, action),
        effects: vec![],
    }
}

fn reduce(state: &mut AppState, action: Action) -> bool {
    match action {
        Action::Quit => false,
        // Middleware supplies the next ring of cells on each animation tick.
        Action::RevealWave => false,
        Action::Reveal(x, y) => {
            state.protect_first_reveal(x, y);
            let Some(cell) = state.grid.get_mut(y).and_then(|row| row.get_mut(x)) else {
                return false;
            };
            cell.revealed = true;

            if cell.mine {
                state.game_state = GameState::Lost;
            } else {
                state.cells_revealed += 1;
                if state.cells_revealed == state.total_safe {
                    state.game_state = GameState::Won;
                }
            }
            true
        }
        Action::ToggleFlag(x, y) => {
            let Some(cell) = state.grid.get_mut(y).and_then(|row| row.get_mut(x)) else {
                return false;
            };
            if cell.flagged {
                cell.flagged = false;
                state.flags_placed = state.flags_placed.saturating_sub(1);
            } else {
                cell.flagged = true;
                state.flags_placed += 1;
            }
            true
        }
        Action::CursorUp => {
            if state.cursor_y > 0 {
                state.cursor_y -= 1;
                true
            } else {
                false
            }
        }
        Action::CursorDown => {
            if state.cursor_y < state.height - 1 {
                state.cursor_y += 1;
                true
            } else {
                false
            }
        }
        Action::CursorLeft => {
            if state.cursor_x > 0 {
                state.cursor_x -= 1;
                true
            } else {
                false
            }
        }
        Action::CursorRight => {
            if state.cursor_x < state.width - 1 {
                state.cursor_x += 1;
                true
            } else {
                false
            }
        }
        Action::SetDifficulty(difficulty) => {
            *state = AppState::new(difficulty);
            true
        }
        Action::NewGame => {
            *state = AppState::new(state.difficulty);
            true
        }
        Action::CustomOpen => {
            state.custom_form = Some(CustomForm::new(state));
            true
        }
        Action::CustomCancel => state.custom_form.take().is_some(),
        Action::CustomField(direction) => {
            let Some(form) = state.custom_form.as_mut() else {
                return false;
            };
            form.field = (form.field as isize + direction as isize).rem_euclid(3) as usize;
            form.replace = true;
            true
        }
        Action::CustomValue(value) => {
            let Some(form) = state.custom_form.as_mut() else {
                return false;
            };
            if value.len() > form.max_digits() || !value.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            form.values[form.field] = value;
            form.replace = false;
            form.error = None;
            true
        }
        Action::CustomCursorMove => {
            let Some(form) = state.custom_form.as_mut() else {
                return false;
            };
            form.replace = false;
            true
        }
        Action::CustomConfirm => {
            let Some(form) = state.custom_form.as_mut() else {
                return false;
            };
            match form.difficulty() {
                Ok(difficulty) => *state = AppState::new(difficulty),
                Err((field, message)) => {
                    form.field = field;
                    form.replace = true;
                    form.error = Some(message.to_string());
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::MinesweeperMiddleware;
    use crate::state::{neighbors, Difficulty};
    use tui_dispatch::prelude::StoreWithMiddleware;

    fn first_mine(state: &AppState) -> (usize, usize) {
        (0..state.height)
            .flat_map(|y| (0..state.width).map(move |x| (x, y)))
            .find(|&(x, y)| state.grid[y][x].mine)
            .unwrap()
    }

    #[test]
    fn every_opening_cell_is_safe_at_every_difficulty() {
        for difficulty in [
            Difficulty::Beginner,
            Difficulty::Intermediate,
            Difficulty::Expert,
        ] {
            let original = AppState::new(difficulty);
            for y in 0..original.height {
                for x in 0..original.width {
                    let mut state = AppState {
                        grid: original.grid.clone(),
                        ..AppState::new(difficulty)
                    };
                    assert!(reducer(&mut state, Action::Reveal(x, y)).changed);
                    assert!(state.grid[y][x].revealed);
                    assert!(!state.grid[y][x].mine);
                    assert!(state.game_state == GameState::Playing);
                    assert_eq!(state.cells_revealed, 1);
                    assert_eq!(
                        state.grid.iter().flatten().filter(|cell| cell.mine).count(),
                        state.mine_count
                    );
                    for cy in 0..state.height {
                        for cx in 0..state.width {
                            let cell = &state.grid[cy][cx];
                            if !cell.mine {
                                let count = neighbors(cx, cy, state.width, state.height)
                                    .iter()
                                    .filter(|&&(nx, ny)| state.grid[ny][nx].mine)
                                    .count();
                                assert_eq!(cell.adjacent as usize, count);
                            }
                            if !original.grid[y][x].mine {
                                assert_eq!(cell.mine, original.grid[cy][cx].mine);
                            }
                        }
                    }

                    // Protection ends after the first successful reveal.
                    let (mx, my) = first_mine(&state);
                    reducer(&mut state, Action::Reveal(mx, my));
                    assert!(state.game_state == GameState::Lost);
                }
            }
        }
    }

    #[test]
    fn flags_and_cancelled_reveals_preserve_opening_protection() {
        let state = AppState::new(Difficulty::Beginner);
        let (x, y) = first_mine(&state);
        let mut store = StoreWithMiddleware::new(state, reducer, MinesweeperMiddleware::default());
        store.dispatch(Action::ToggleFlag(x, y));
        store.dispatch(Action::Reveal(x, y));
        store.dispatch(Action::Reveal(usize::MAX, usize::MAX));
        assert_eq!(store.state().cells_revealed, 0);
        assert_eq!(store.state().flags_placed, 1);
        assert!(store.state().grid[y][x].mine);

        store.dispatch(Action::ToggleFlag(x, y));
        store.dispatch(Action::Reveal(x, y));
        assert!(store.state().grid[y][x].revealed);
        assert!(!store.state().grid[y][x].mine);
        assert!(store.state().game_state != GameState::Lost);
        assert!(!store
            .state()
            .grid
            .iter()
            .flatten()
            .any(|cell| cell.mine && cell.revealed));
    }

    #[test]
    fn new_game_and_difficulty_changes_restore_opening_protection() {
        let mut store = StoreWithMiddleware::new(
            AppState::new(Difficulty::Beginner),
            reducer,
            MinesweeperMiddleware::default(),
        );
        for reset in [
            Action::NewGame,
            Action::SetDifficulty(Difficulty::Intermediate),
            Action::SetDifficulty(Difficulty::Expert),
        ] {
            store.dispatch(reset);
            assert_eq!(store.state().cells_revealed, 0);
            let (x, y) = first_mine(store.state());
            store.dispatch(Action::Reveal(x, y));
            assert!(store.state().grid[y][x].revealed);
            assert!(!store.state().grid[y][x].mine);
            assert!(store.state().game_state != GameState::Lost);
        }
    }
}
