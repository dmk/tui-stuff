use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::MapState;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HunterState {
    pub x: u16,
    pub y: u16,
    pub step_interval: u8,
    pub step_cooldown: u8,
    pub hearing_radius: u16,
}

impl HunterState {
    pub fn new(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            step_interval: 2,
            step_cooldown: 0,
            hearing_radius: 8,
        }
    }
}

pub fn ready_to_advance(state: &mut HunterState) -> bool {
    if state.step_cooldown == 0 {
        state.step_cooldown = state.step_interval.saturating_sub(1);
        true
    } else {
        state.step_cooldown = state.step_cooldown.saturating_sub(1);
        false
    }
}

pub fn advance_toward(state: &mut HunterState, target: (u16, u16), map: &MapState) {
    let mut candidates = [
        (state.x, state.y.saturating_sub(1)),
        (state.x, state.y.saturating_add(1)),
        (state.x.saturating_sub(1), state.y),
        (state.x.saturating_add(1), state.y),
    ];

    candidates.sort_by_key(|(x, y)| manhattan(*x, *y, target.0, target.1));

    for (x, y) in candidates {
        if map.is_walkable(x, y) {
            state.x = x;
            state.y = y;
            return;
        }
    }
}

fn manhattan(x0: u16, y0: u16, x1: u16, y1: u16) -> u32 {
    x0.abs_diff(x1) as u32 + y0.abs_diff(y1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MapState;
    use tui_map::core::{MapSize, TileKind};

    #[test]
    fn hunter_moves_toward_target_on_open_map() {
        let map = MapState::filled("test", MapSize::new(8, 8), TileKind::Floor);
        let mut hunter = HunterState::new(1, 1);
        advance_toward(&mut hunter, (5, 1), &map);
        assert_eq!((hunter.x, hunter.y), (2, 1));
    }
}
