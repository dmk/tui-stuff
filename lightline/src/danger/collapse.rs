use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::TrailState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CollapsePhase {
    Dormant,
    Warning,
    Active,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CollapseState {
    pub phase: CollapsePhase,
    pub trigger_step: u32,
    pub warning_steps_left: u8,
    pub frontier: Vec<(u16, u16)>,
}

impl CollapseState {
    pub fn new(trigger_step: u32, warning_steps: u8) -> Self {
        Self {
            phase: CollapsePhase::Dormant,
            trigger_step,
            warning_steps_left: warning_steps,
            frontier: Vec::new(),
        }
    }

    pub fn maybe_start_warning(&mut self, steps_taken: u32) -> bool {
        if self.phase == CollapsePhase::Dormant && steps_taken >= self.trigger_step {
            self.phase = CollapsePhase::Warning;
            return true;
        }
        false
    }

    pub fn tick_warning(&mut self) -> bool {
        if self.phase != CollapsePhase::Warning {
            return false;
        }

        if self.warning_steps_left > 0 {
            self.warning_steps_left -= 1;
        }

        if self.warning_steps_left == 0 {
            self.phase = CollapsePhase::Active;
            return true;
        }

        false
    }

    pub fn cut_trail_at(&self, trail: &mut TrailState, x: u16, y: u16) -> u16 {
        trail.take(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_transitions_from_warning_to_active() {
        let mut state = CollapseState::new(5, 2);
        assert!(state.maybe_start_warning(5));
        assert_eq!(state.phase, CollapsePhase::Warning);
        assert!(!state.tick_warning());
        assert!(state.tick_warning());
        assert_eq!(state.phase, CollapsePhase::Active);
    }
}
