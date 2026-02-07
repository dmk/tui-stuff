use std::collections::VecDeque;

use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::{AppState, Direction, GameMode, RuntimeAnchorKind, TrailState};

const BASE_WIDTH: u16 = 36;
const BASE_HEIGHT: u16 = 24;
const MAX_WIDTH: u16 = 64;
const MAX_HEIGHT: u16 = 40;
// Movement/lightline tunables:
// - STEP_BURN: light spent moving onto an unlit tile.
// - LIT_STEP_BURN: light spent moving onto an already lit tile.
// - TRAIL_DEPOSIT_AMOUNT: charge laid when leaving an unlit tile.
const STEP_BURN: u16 = 1;
const LIT_STEP_BURN: u16 = 0;
const TRAIL_DEPOSIT_AMOUNT: u16 = 1;
const START_LIGHT: u16 = 120;
const LIGHT_DECAY_EVERY: u32 = 3;
const LIGHT_DECAY_AMOUNT: u16 = 3;
const MIN_START_LIGHT: u16 = 75;

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Init => {
            state.floor_index = 0;
            state.player.steps = 0;
            state.last_status = Some("New run started.".to_string());
            state.mode = GameMode::Boot;
            DispatchResult::changed_with(generate_floor_effect(state.floor_index, state.seed))
        }
        Action::GenerateFloor => {
            state.mode = GameMode::Boot;
            DispatchResult::changed_with(generate_floor_effect(state.floor_index, state.seed))
        }
        Action::FloorGenerated(floor) => {
            let starting_light = light_budget_for_floor(state.floor_index);
            state.player.light_max = starting_light;
            state.player.light_current = starting_light;
            state.apply_generated_floor(floor);
            DispatchResult::changed()
        }
        Action::Move(direction, collect) => handle_move(state, direction, collect),
        Action::Descend => {
            state.floor_index = state.floor_index.saturating_add(1);
            state.mode = GameMode::Boot;
            DispatchResult::changed_with(generate_floor_effect(state.floor_index, state.seed))
        }
        Action::Interact => {
            state.last_status = Some("No interactive object on this tile yet.".to_string());
            DispatchResult::changed()
        }
        Action::Tick | Action::DangerAdvance => DispatchResult::unchanged(),
        Action::GameOver => {
            set_game_over(state, "Light exhausted.");
            DispatchResult::changed()
        }
        Action::PauseOpen => {
            if state.mode == GameMode::Exploration {
                state.mode = GameMode::Pause;
                return DispatchResult::changed();
            }
            DispatchResult::unchanged()
        }
        Action::PauseClose => {
            if state.mode == GameMode::Pause {
                state.mode = GameMode::Exploration;
                return DispatchResult::changed();
            }
            DispatchResult::unchanged()
        }
        Action::Quit => DispatchResult::unchanged(),
    }
}

fn trail_has_charge_after_move(
    trail: &TrailState,
    curr: (u16, u16),
    curr_has_charge_after_move: bool,
    x: u16,
    y: u16,
) -> bool {
    if (x, y) == curr {
        return curr_has_charge_after_move;
    }
    trail.charge_at(x, y) > 0
}

fn lightline_preserved_after_move(
    state: &AppState,
    curr: (u16, u16),
    next: (u16, u16),
    curr_has_charge_after_move: bool,
) -> bool {
    let start = state
        .anchor_pos(RuntimeAnchorKind::PlayerStart)
        .unwrap_or(curr);
    if !state.map.is_walkable(start.0, start.1) || !state.map.is_walkable(next.0, next.1) {
        return false;
    }

    let mut visited = vec![false; state.map.width as usize * state.map.height as usize];
    let mut queue = VecDeque::from([start]);

    while let Some((x, y)) = queue.pop_front() {
        let idx = y as usize * state.map.width as usize + x as usize;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        for (dx, dy) in [(0i16, -1), (0, 1), (-1, 0), (1, 0)] {
            let nx = x as i16 + dx;
            let ny = y as i16 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            if nx >= state.map.width || ny >= state.map.height {
                continue;
            }
            if !state.map.is_walkable(nx, ny) {
                continue;
            }
            if (nx, ny) != next
                && !trail_has_charge_after_move(
                    &state.trail,
                    curr,
                    curr_has_charge_after_move,
                    nx,
                    ny,
                )
            {
                continue;
            }

            let nidx = ny as usize * state.map.width as usize + nx as usize;
            if !visited[nidx] {
                queue.push_back((nx, ny));
            }
        }
    }

    let next_idx = next.1 as usize * state.map.width as usize + next.0 as usize;
    if !visited[next_idx] {
        return false;
    }

    for y in 0..state.map.height {
        for x in 0..state.map.width {
            if !trail_has_charge_after_move(&state.trail, curr, curr_has_charge_after_move, x, y) {
                continue;
            }
            let idx = y as usize * state.map.width as usize + x as usize;
            if !visited[idx] {
                return false;
            }
        }
    }

    true
}

fn handle_move(
    state: &mut AppState,
    direction: Direction,
    collect: bool,
) -> DispatchResult<Effect> {
    if state.mode != GameMode::Exploration {
        return DispatchResult::unchanged();
    }

    let (curr_x, curr_y) = state.player_pos();
    let (mut next_x, mut next_y) = (curr_x, curr_y);
    match direction {
        Direction::Up => next_y = next_y.saturating_sub(1),
        Direction::Down => next_y = next_y.saturating_add(1),
        Direction::Left => next_x = next_x.saturating_sub(1),
        Direction::Right => next_x = next_x.saturating_add(1),
    }

    if (next_x, next_y) == (curr_x, curr_y) {
        return DispatchResult::unchanged();
    }
    if !state.map.is_walkable(next_x, next_y) {
        state.last_status = Some("Blocked path.".to_string());
        return DispatchResult::unchanged();
    }

    let curr_had_trail = state.trail.charge_at(curr_x, curr_y) > 0;
    let will_collect = collect && curr_had_trail;
    // After this move, current tile has charge iff we are doing a normal move.
    // Shift+move never leaves charge on the current tile.
    let curr_has_charge_after_move = !collect;
    if !lightline_preserved_after_move(
        state,
        (curr_x, curr_y),
        (next_x, next_y),
        curr_has_charge_after_move,
    ) {
        state.last_status = Some("Illegal move: would break the lightline to start.".to_string());
        return DispatchResult::unchanged();
    }

    let reclaimed = if will_collect {
        // Shift+move: pick up current tile's trail charge.
        state.trail.take(curr_x, curr_y)
    } else {
        0
    };

    // Normal movement lays trail once per tile.
    if !collect && state.trail.charge_at(curr_x, curr_y) == 0 {
        state.trail.deposit(curr_x, curr_y, TRAIL_DEPOSIT_AMOUNT);
    }

    let burn_cost = if state.trail.charge_at(next_x, next_y) > 0 {
        LIT_STEP_BURN
    } else {
        STEP_BURN
    };

    state.player.x = next_x;
    state.player.y = next_y;

    state.player.light_current = state
        .player
        .light_current
        .saturating_sub(burn_cost)
        .saturating_add(reclaimed)
        .min(state.player.light_max);
    state.player.steps = state.player.steps.saturating_add(1);

    if state.player.light_current == 0 {
        set_game_over(state, "Your lantern goes dark.");
        return DispatchResult::changed();
    }

    if is_anchor(state, RuntimeAnchorKind::Exit, next_x, next_y) {
        state.floor_index = state.floor_index.saturating_add(1);
        state.mode = GameMode::Boot;
        state.last_status = Some(format!("Descended to floor {}", state.floor_index + 1));
        return DispatchResult::changed_with(generate_floor_effect(state.floor_index, state.seed));
    }

    state.last_status = Some(format!("Steps: {}", state.player.steps));
    DispatchResult::changed()
}

fn generate_floor_effect(floor_index: u32, seed: u64) -> Effect {
    let (width, height) = floor_dimensions(floor_index);
    Effect::GenerateFloor {
        floor_index,
        seed,
        width,
        height,
    }
}

fn floor_dimensions(floor_index: u32) -> (u16, u16) {
    let growth = (floor_index / 2) as u16;
    (
        BASE_WIDTH.saturating_add(growth * 2).min(MAX_WIDTH),
        BASE_HEIGHT.saturating_add(growth * 2).min(MAX_HEIGHT),
    )
}

fn light_budget_for_floor(floor_index: u32) -> u16 {
    let decay_steps = floor_index / LIGHT_DECAY_EVERY;
    START_LIGHT
        .saturating_sub(decay_steps as u16 * LIGHT_DECAY_AMOUNT)
        .max(MIN_START_LIGHT)
}

fn is_anchor(state: &AppState, kind: RuntimeAnchorKind, x: u16, y: u16) -> bool {
    state
        .anchors
        .iter()
        .any(|anchor| anchor.kind == kind && anchor.x == x && anchor.y == y)
}

fn set_game_over(state: &mut AppState, message: &str) {
    state.mode = GameMode::GameOver;
    state.last_status = Some(message.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procgen::generate_floor;

    fn walkable_neighbor(state: &AppState, x: u16, y: u16) -> Option<(Direction, u16, u16)> {
        let candidates = [
            (Direction::Right, x + 1, y),
            (Direction::Left, x.saturating_sub(1), y),
            (Direction::Down, x, y + 1),
            (Direction::Up, x, y.saturating_sub(1)),
        ];
        candidates
            .into_iter()
            .find(|(_, nx, ny)| state.map.is_walkable(*nx, *ny))
    }

    fn opposite(dir: Direction) -> Direction {
        match dir {
            Direction::Right => Direction::Left,
            Direction::Left => Direction::Right,
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
        }
    }

    #[test]
    fn moving_consumes_light() {
        let mut state = AppState::new(123);
        state.player.light_current = 10;
        state.player.light_max = 10;
        let floor = generate_floor(123, 0, 36, 24).expect("floor");
        state.apply_generated_floor(floor);

        let (x, y) = state.player_pos();
        let (dir, ex, ey) = walkable_neighbor(&state, x, y).expect("walkable neighbor");
        let _ = reducer(&mut state, Action::Move(dir, false));
        assert_eq!(state.player_pos(), (ex, ey));
        assert_eq!(state.player.light_current, 9);
    }

    #[test]
    fn walking_on_lit_trail_does_not_consume_light() {
        let mut state = AppState::new(456);
        state.player.light_current = 10;
        state.player.light_max = 10;
        let floor = generate_floor(456, 0, 36, 24).expect("floor");
        state.apply_generated_floor(floor);

        let (x, y) = state.player_pos();
        let (dir, _, _) = walkable_neighbor(&state, x, y).expect("walkable neighbor");

        // Normal move: deposits 1 on start tile, burns 1 light
        let _ = reducer(&mut state, Action::Move(dir, false));
        assert_eq!(state.player.light_current, 9);
        assert_eq!(state.trail.charge_at(x, y), 1);

        // Move back onto already lit tile: no burn.
        let _ = reducer(&mut state, Action::Move(opposite(dir), false));
        assert_eq!(state.player.light_current, 9);

        // Shift-modified move collects the current trail charge.
        let _ = reducer(&mut state, Action::Move(dir, true));
        assert_eq!(state.player.light_current, 10);
        assert_eq!(state.trail.charge_at(x, y), 0);
    }

    #[test]
    fn trail_deposit_caps_at_one() {
        let mut state = AppState::new(456);
        state.player.light_current = 10;
        state.player.light_max = 10;
        let floor = generate_floor(456, 0, 36, 24).expect("floor");
        state.apply_generated_floor(floor);

        let (x, y) = state.player_pos();
        let (dir, _, _) = walkable_neighbor(&state, x, y).expect("walkable neighbor");

        // Move away and back twice — trail should still be 1, not 2
        let _ = reducer(&mut state, Action::Move(dir, false));
        let _ = reducer(&mut state, Action::Move(opposite(dir), false));
        let _ = reducer(&mut state, Action::Move(dir, false));
        assert_eq!(state.trail.charge_at(x, y), 1);
    }

    #[test]
    fn reaching_exit_triggers_descend() {
        let mut state = AppState::new(789);
        state.player.light_current = 20;
        state.player.light_max = 20;
        let floor = generate_floor(789, 0, 36, 24).expect("floor");
        state.apply_generated_floor(floor);

        let (exit_x, exit_y) = state.exit_pos().expect("exit");
        // Find a walkable tile adjacent to exit and the direction to reach exit from it
        let approach = [
            (Direction::Right, exit_x.saturating_sub(1), exit_y),
            (Direction::Left, exit_x + 1, exit_y),
            (Direction::Down, exit_x, exit_y.saturating_sub(1)),
            (Direction::Up, exit_x, exit_y + 1),
        ];
        let (dir, adj_x, adj_y) = approach
            .into_iter()
            .find(|(_, ax, ay)| state.map.is_walkable(*ax, *ay))
            .expect("exit has walkable neighbor");

        state.player.x = adj_x;
        state.player.y = adj_y;
        if let Some(start) = state
            .anchors
            .iter_mut()
            .find(|a| a.kind == RuntimeAnchorKind::PlayerStart)
        {
            start.x = adj_x;
            start.y = adj_y;
        }

        let _ = reducer(&mut state, Action::Move(dir, false));
        assert_eq!(state.floor_index, 1);
        assert_eq!(state.mode, GameMode::Boot);
    }

    #[test]
    fn illegal_move_when_lightline_is_disconnected_from_start() {
        use crate::state::{MapState, RuntimeAnchor, Tile};
        use tui_map::core::{MapSize, TileKind};

        let mut state = AppState::new(999);
        state.mode = GameMode::Exploration;
        state.map = MapState::filled("test", MapSize::new(5, 3), TileKind::Floor);
        state.trail = TrailState::new(5, 3);
        state.anchors = vec![RuntimeAnchor {
            kind: RuntimeAnchorKind::PlayerStart,
            x: 0,
            y: 1,
            tag: None,
        }];
        state.player.x = 3;
        state.player.y = 1;
        state.player.light_current = 10;
        state.player.light_max = 10;
        state.player.steps = 5;
        // Only current tile is lit; no connected path to start.
        state.trail.deposit(3, 1, 1);

        // Ensure destination is walkable.
        let idx = 1usize * state.map.width as usize + 4usize;
        state.map.tiles[idx] = Tile::Floor;

        let _ = reducer(&mut state, Action::Move(Direction::Right, false));
        assert_eq!(state.player_pos(), (3, 1));
        assert_eq!(state.player.light_current, 10);
        assert_eq!(
            state.last_status.as_deref(),
            Some("Illegal move: would break the lightline to start.")
        );
    }

    #[test]
    fn collect_move_is_illegal_if_it_disconnects_chain_to_start() {
        use crate::state::{MapState, RuntimeAnchor, Tile};
        use tui_map::core::{MapSize, TileKind};

        let mut state = AppState::new(1001);
        state.mode = GameMode::Exploration;
        state.map = MapState::filled("line", MapSize::new(5, 1), TileKind::Floor);
        state.trail = TrailState::new(5, 1);
        state.anchors = vec![RuntimeAnchor {
            kind: RuntimeAnchorKind::PlayerStart,
            x: 0,
            y: 0,
            tag: None,
        }];
        state.player.x = 2;
        state.player.y = 0;
        state.player.light_current = 10;
        state.player.light_max = 10;
        state.player.steps = 4;

        // Continuous trail from start to the right side.
        for x in 0..=3u16 {
            state.trail.deposit(x, 0, 1);
        }

        // Ensure destination is walkable.
        state.map.tiles[3] = Tile::Floor;

        // Collecting at x=2 would remove bridge segment and disconnect x=3 from start.
        let _ = reducer(&mut state, Action::Move(Direction::Right, true));
        assert_eq!(state.player_pos(), (2, 0));
        assert_eq!(state.trail.charge_at(2, 0), 1);
        assert_eq!(
            state.last_status.as_deref(),
            Some("Illegal move: would break the lightline to start.")
        );
    }

    #[test]
    fn collect_from_unlit_tile_does_not_count_as_chain_segment() {
        use crate::state::{MapState, RuntimeAnchor};
        use tui_map::core::{MapSize, TileKind};

        let mut state = AppState::new(1002);
        state.mode = GameMode::Exploration;
        state.map = MapState::filled("line", MapSize::new(4, 1), TileKind::Floor);
        state.trail = TrailState::new(4, 1);
        state.anchors = vec![RuntimeAnchor {
            kind: RuntimeAnchorKind::PlayerStart,
            x: 0,
            y: 0,
            tag: None,
        }];
        state.player.x = 1;
        state.player.y = 0;
        state.player.light_current = 10;
        state.player.light_max = 10;
        state.player.steps = 1;
        // Current tile has no trail charge.

        // Shift+move should be illegal here because current tile won't stay connected.
        let _ = reducer(&mut state, Action::Move(Direction::Right, true));
        assert_eq!(state.player_pos(), (1, 0));
        assert_eq!(
            state.last_status.as_deref(),
            Some("Illegal move: would break the lightline to start.")
        );
    }

    #[test]
    fn move_is_illegal_if_it_disconnects_any_lit_segment() {
        use crate::state::{MapState, RuntimeAnchor};
        use tui_map::core::{MapSize, TileKind};

        let mut state = AppState::new(1003);
        state.mode = GameMode::Exploration;
        state.map = MapState::filled("grid", MapSize::new(4, 3), TileKind::Floor);
        state.trail = TrailState::new(4, 3);
        state.anchors = vec![RuntimeAnchor {
            kind: RuntimeAnchorKind::PlayerStart,
            x: 0,
            y: 1,
            tag: None,
        }];
        state.player.x = 1;
        state.player.y = 1;
        state.player.light_current = 10;
        state.player.light_max = 10;
        state.player.steps = 3;

        // Alternate route from start to destination (2,1) that does not use current (1,1).
        for &(x, y) in &[(0, 1), (0, 0), (1, 0), (2, 0), (2, 1)] {
            state.trail.deposit(x, y, 1);
        }
        // Current tile has charge and supports this side segment.
        state.trail.deposit(1, 1, 1);
        state.trail.deposit(1, 2, 1);

        // Shift+move right would keep destination connected, but strand (1,2).
        let _ = reducer(&mut state, Action::Move(Direction::Right, true));
        assert_eq!(state.player_pos(), (1, 1));
        assert_eq!(
            state.last_status.as_deref(),
            Some("Illegal move: would break the lightline to start.")
        );
        // Collecting did not happen because the move was rejected.
        assert_eq!(state.trail.charge_at(1, 1), 1);
    }
}
