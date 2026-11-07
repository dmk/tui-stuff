use std::collections::HashSet;

use tui_dispatch::prelude::*;

use crate::action::Action;
use crate::state::{neighbors, AppState, GameState};

pub const REVEAL_TICK_MS: u64 = 40;

/// Enforces game rules and reveals empty areas one neighboring ring per tick.
#[derive(Default)]
pub struct MinesweeperMiddleware {
    pending: Vec<(usize, usize)>,
    scheduled: HashSet<(usize, usize)>,
}

impl MinesweeperMiddleware {
    fn clear(&mut self) {
        self.pending.clear();
        self.scheduled.clear();
    }
}

impl Middleware<AppState, Action> for MinesweeperMiddleware {
    fn before(&mut self, action: &Action, state: &AppState) -> bool {
        if state.custom_form.is_some()
            && matches!(
                action,
                Action::Reveal(..) | Action::ToggleFlag(..) | Action::RevealWave
            )
        {
            return false;
        }
        match action {
            Action::NewGame | Action::SetDifficulty(_) => self.clear(),
            Action::CustomConfirm
                if state
                    .custom_form
                    .as_ref()
                    .is_some_and(|form| form.difficulty().is_ok()) =>
            {
                self.clear()
            }
            Action::Reveal(x, y) => {
                self.scheduled.remove(&(*x, *y));
            }
            _ => {}
        }

        match action {
            Action::Reveal(x, y) => {
                state
                    .grid
                    .get(*y)
                    .and_then(|row| row.get(*x))
                    .is_some_and(|cell| {
                        state.game_state == GameState::Playing && !cell.revealed && !cell.flagged
                    })
            }
            Action::ToggleFlag(x, y) => state
                .grid
                .get(*y)
                .and_then(|row| row.get(*x))
                .is_some_and(|cell| state.game_state == GameState::Playing && !cell.revealed),
            _ => true,
        }
    }

    fn after(&mut self, action: &Action, state_changed: bool, state: &AppState) -> Vec<Action> {
        if state.game_state != GameState::Playing {
            self.clear();
            return vec![];
        }

        match action {
            Action::RevealWave => {
                // Snapshot this frontier. Reveals queue their neighbors for the
                // next tick, so no cascade can finish within a single frame.
                return self
                    .pending
                    .drain(..)
                    .map(|(x, y)| Action::Reveal(x, y))
                    .collect();
            }
            Action::Reveal(x, y) if state_changed && state.grid[*y][*x].adjacent == 0 => {
                for (nx, ny) in neighbors(*x, *y, state.width, state.height) {
                    let cell = &state.grid[ny][nx];
                    if !cell.revealed
                        && !cell.flagged
                        && !cell.mine
                        && self.scheduled.insert((nx, ny))
                    {
                        self.pending.push((nx, ny));
                    }
                }
            }
            _ => {}
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::reducer;
    use crate::state::{Cell, Difficulty};

    fn board() -> StoreWithMiddleware<AppState, Action, NoEffect, MinesweeperMiddleware> {
        let mut state = AppState::new(Difficulty::Beginner);
        state.width = 5;
        state.height = 5;
        state.grid = vec![vec![Cell::default(); 5]; 5];
        state.grid[4][4].mine = true;
        for (x, y) in neighbors(4, 4, 5, 5) {
            state.grid[y][x].adjacent = 1;
        }
        state.mine_count = 1;
        state.total_safe = 24;
        StoreWithMiddleware::new(state, reducer, MinesweeperMiddleware::default())
    }

    #[test]
    fn empty_area_expands_in_rings_and_wins_only_after_the_last_wave() {
        let mut store = board();
        store.dispatch(Action::Reveal(0, 0));
        assert_eq!(store.state().cells_revealed, 1);
        for radius in 1..=3 {
            assert!(store.dispatch(Action::RevealWave).changed);
            assert_eq!(store.state().cells_revealed, (radius + 1) * (radius + 1));
            for y in 0..5 {
                for x in 0..5 {
                    assert_eq!(
                        store.state().grid[y][x].revealed,
                        x <= radius && y <= radius
                    );
                }
            }
            assert!(store.state().game_state == GameState::Playing);
        }
        assert!(store.dispatch(Action::RevealWave).changed);
        assert_eq!(store.state().cells_revealed, 24);
        assert!(store.state().game_state == GameState::Won);
        assert!(!store.state().grid[4][4].revealed);
        assert!(!store.dispatch(Action::RevealWave).changed);
    }

    #[test]
    fn flags_and_manual_clicks_are_respected_during_a_wave() {
        let mut store = board();
        store.dispatch(Action::Reveal(0, 0));
        store.dispatch(Action::ToggleFlag(1, 1));
        store.dispatch(Action::Reveal(1, 0));
        for _ in 0..10 {
            store.dispatch(Action::RevealWave);
        }
        assert!(!store.state().grid[1][1].revealed);
        assert_eq!(store.state().cells_revealed, 23);
        assert!(store.state().game_state == GameState::Playing);
        store.dispatch(Action::ToggleFlag(1, 1));
        store.dispatch(Action::Reveal(1, 1));
        assert_eq!(store.state().cells_revealed, 24);
        assert!(store.state().game_state == GameState::Won);
    }

    #[test]
    fn custom_modal_pauses_and_cancels_or_replaces_the_board() {
        let mut store = board();
        store.dispatch(Action::Reveal(0, 0));
        store.dispatch(Action::CustomOpen);
        assert!(!store.dispatch(Action::RevealWave).changed);
        assert!(!store.dispatch(Action::Reveal(1, 1)).changed);
        assert!(!store.dispatch(Action::ToggleFlag(1, 1)).changed);
        assert_eq!(store.state().cells_revealed, 1);
        store.dispatch(Action::CustomCancel);
        assert!(store.dispatch(Action::RevealWave).changed);
        assert_eq!(store.state().cells_revealed, 4);
        store.dispatch(Action::CustomOpen);
        store.dispatch(Action::CustomConfirm);
        assert!(!store.dispatch(Action::RevealWave).changed);
        assert_eq!(store.state().cells_revealed, 0);
    }

    #[test]
    fn game_over_and_resets_cancel_pending_waves() {
        let mut store = board();
        store.dispatch(Action::Reveal(0, 0));
        store.dispatch(Action::Reveal(4, 4));
        assert!(store.state().game_state == GameState::Lost);
        assert!(!store.dispatch(Action::RevealWave).changed);
        assert_eq!(store.state().cells_revealed, 1);

        for reset in [Action::NewGame, Action::SetDifficulty(Difficulty::Expert)] {
            let mut store = board();
            store.dispatch(Action::Reveal(0, 0));
            store.dispatch(reset);
            assert!(!store.dispatch(Action::RevealWave).changed);
            assert_eq!(store.state().cells_revealed, 0);
        }

        let mut store = board();
        store.dispatch(Action::Reveal(3, 3));
        assert!(!store.dispatch(Action::RevealWave).changed);
        assert_eq!(store.state().cells_revealed, 1);
    }
}
