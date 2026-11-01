use tui_dispatch::ReducerResult;

use crate::{action::Action, state::*};

pub fn reducer(state: &mut AppState, action: Action) -> ReducerResult {
    match action {
        Action::Tick => {
            if !state.running || state.show_help || !state.ready() {
                return ReducerResult::unchanged();
            }
            state.clock += state.speed();
            while state.clock >= 50 {
                state.clock -= 50;
                evolve(state);
            }
        }
        Action::PauseToggle => {
            state.running = !state.running;
            state.clock = 0;
        }
        Action::Step | Action::Advance(_) => {
            // A replay can explicitly advance a paused simulation, exactly as N does.
            let count = if let Action::Advance(count) = action {
                count
            } else {
                1
            };
            state.running = false;
            state.clock = 0;
            for _ in 0..count {
                evolve(state);
            }
        }
        Action::SpeedChange(delta) => {
            state.speed_index = (state.speed_index as i32 + i32::from(delta))
                .clamp(0, SPEEDS.len() as i32 - 1) as usize;
        }
        Action::PatternSelect(pattern) => state.selected = pattern,
        Action::PatternLoad => {
            state.reset_board();
            state.home();
            state.stamp(state.selected, state.cursor_x, state.cursor_y);
            state.scene = state.selected.name().into();
            state.recount();
            state.running = false;
        }
        Action::PatternStamp => {
            state.stamp(state.selected, state.cursor_x, state.cursor_y);
            edited(state);
        }
        Action::CellSet { x, y, alive } => {
            let Some(i) = AppState::index(x, y) else {
                return ReducerResult::unchanged();
            };
            state.cells[i] = u16::from(alive);
            state.trails[i] = 0;
            state.cursor_x = x;
            state.cursor_y = y;
            state.cursor_visible = true;
            edited(state);
        }
        Action::CellToggle => {
            let i = AppState::index(state.cursor_x, state.cursor_y).unwrap();
            state.cells[i] = u16::from(state.cells[i] == 0);
            state.trails[i] = 0;
            state.cursor_visible = true;
            edited(state);
        }
        Action::CursorMove { dx, dy } => {
            state.cursor_x = state.cursor_x.saturating_add(dx).clamp(0, WORLD_WIDTH - 1);
            state.cursor_y = state.cursor_y.saturating_add(dy).clamp(0, WORLD_HEIGHT - 1);
            state.cursor_visible = true;
            // Follow the cursor so keyboard editing also works outside the initial view.
            let view = crate::ui::BoardView::for_state(state);
            if state.cursor_x < view.origin_x + 2 || state.cursor_x >= view.origin_x + view.cols - 2
            {
                state.camera_x = state.cursor_x;
            }
            if state.cursor_y < view.origin_y + 2 || state.cursor_y >= view.origin_y + view.rows - 2
            {
                state.camera_y = state.cursor_y;
            }
        }
        Action::CameraPan { dx, dy } => {
            state.camera_x = state.camera_x.saturating_add(dx).clamp(0, WORLD_WIDTH - 1);
            state.camera_y = state.camera_y.saturating_add(dy).clamp(0, WORLD_HEIGHT - 1);
        }
        Action::CameraHome => state.home(),
        Action::ZoomCycle => state.zoom = (state.zoom + 1) % 3,
        Action::Clear => {
            state.reset_board();
            state.recount();
            state.scene = "Blank canvas".into();
            state.running = false;
        }
        Action::Randomize => {
            state.reset_board();
            state.home();
            for cell in &mut state.cells {
                // SplitMix64; seed zero is supported and every draw is replayable.
                state.seed = state.seed.wrapping_add(0x9E3779B97F4A7C15);
                let mut value = state.seed;
                value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
                value ^= value >> 31;
                *cell = u16::from(value % 100 < 24);
            }
            state.recount();
            state.scene = "Primordial soup".into();
            state.running = false;
        }
        Action::BoundaryToggle => state.wrapping = !state.wrapping,
        Action::GridToggle => state.show_grid = !state.show_grid,
        Action::TrailsToggle => state.show_trails = !state.show_trails,
        Action::HelpToggle => state.show_help = !state.show_help,
        Action::ViewportResize { width, height } => {
            state.viewport_width = width;
            state.viewport_height = height;
        }
        Action::Quit => return ReducerResult::unchanged(),
    }
    ReducerResult::changed()
}

fn edited(state: &mut AppState) {
    state.running = false;
    state.clock = 0;
    state.births = 0;
    state.deaths = 0;
    state.scene = "Custom garden".into();
    state.recount();
}

/// Conway B3/S23, using a separate next generation to keep updates simultaneous.
pub fn evolve(state: &mut AppState) {
    let mut neighbors = vec![0u8; CELL_COUNT];
    for (i, &age) in state.cells.iter().enumerate() {
        if age == 0 {
            continue;
        }
        let (x, y) = (i as i32 % WORLD_WIDTH, i as i32 / WORLD_WIDTH);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = if state.wrapping {
                    (
                        (x + dx).rem_euclid(WORLD_WIDTH),
                        (y + dy).rem_euclid(WORLD_HEIGHT),
                    )
                } else {
                    (x + dx, y + dy)
                };
                if let Some(index) = AppState::index(nx, ny) {
                    neighbors[index] += 1;
                }
            }
        }
    }
    let mut population = 0;
    let mut births = 0;
    let mut deaths = 0;
    for (i, &count) in neighbors.iter().enumerate() {
        let old = state.cells[i];
        let alive = count == 3 || (old > 0 && count == 2);
        if alive {
            state.cells[i] = old.saturating_add(1);
            state.trails[i] = 0;
            population += 1;
            births += u64::from(old == 0);
        } else {
            state.cells[i] = 0;
            state.trails[i] = if old > 0 {
                12
            } else {
                state.trails[i].saturating_sub(1)
            };
            deaths += u64::from(old > 0);
        }
    }
    state.population = population;
    state.births = births;
    state.deaths = deaths;
    state.generation = state.generation.saturating_add(1);
    state.history.push(population);
    if state.history.len() > 80 {
        state.history.remove(0);
    }
}
