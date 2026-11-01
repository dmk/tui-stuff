use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{patterns::Pattern, DEFAULT_SEED};

pub const WORLD_WIDTH: i32 = 192;
pub const WORLD_HEIGHT: i32 = 128;
pub const CELL_COUNT: usize = (WORLD_WIDTH * WORLD_HEIGHT) as usize;
pub const SPEEDS: [u16; 8] = [1, 2, 4, 8, 12, 20, 30, 60];
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 22;

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, tui_dispatch::DebugState,
)]
pub struct AppState {
    /// Zero means dead; living cells store their age in generations.
    #[debug(skip)]
    pub cells: Vec<u16>,
    #[debug(skip)]
    pub trails: Vec<u8>,
    pub generation: u64,
    pub population: u64,
    pub births: u64,
    pub deaths: u64,
    #[debug(debug_fmt)]
    pub history: Vec<u64>,
    pub running: bool,
    pub speed_index: usize,
    /// Fixed-point generation accumulator, in fiftieths of a generation.
    pub clock: u16,
    pub wrapping: bool,
    pub show_grid: bool,
    pub show_trails: bool,
    pub show_help: bool,
    #[debug(debug_fmt)]
    pub selected: Pattern,
    pub scene: String,
    pub camera_x: i32,
    pub camera_y: i32,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub cursor_visible: bool,
    pub zoom: usize,
    pub seed: u64,
    pub viewport_width: u16,
    pub viewport_height: u16,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(DEFAULT_SEED)
    }
}

impl AppState {
    pub fn new(seed: u64) -> Self {
        let mut state = Self {
            cells: vec![0; CELL_COUNT],
            trails: vec![0; CELL_COUNT],
            generation: 0,
            population: 0,
            births: 0,
            deaths: 0,
            history: Vec::new(),
            running: true,
            speed_index: 3,
            clock: 0,
            wrapping: true,
            show_grid: true,
            show_trails: true,
            show_help: false,
            selected: Pattern::Garden,
            scene: Pattern::Garden.name().into(),
            camera_x: WORLD_WIDTH / 2,
            camera_y: WORLD_HEIGHT / 2,
            cursor_x: WORLD_WIDTH / 2,
            cursor_y: WORLD_HEIGHT / 2,
            cursor_visible: false,
            zoom: 1,
            seed,
            viewport_width: 120,
            viewport_height: 42,
        };
        state.stamp(Pattern::Garden, WORLD_WIDTH / 2, WORLD_HEIGHT / 2);
        state.recount();
        state
    }

    pub fn index(x: i32, y: i32) -> Option<usize> {
        (0..WORLD_WIDTH).contains(&x).then_some(())?;
        (0..WORLD_HEIGHT).contains(&y).then_some(())?;
        Some((y * WORLD_WIDTH + x) as usize)
    }

    pub fn age(&self, x: i32, y: i32) -> u16 {
        Self::index(x, y).map_or(0, |i| self.cells[i])
    }

    pub fn ready(&self) -> bool {
        self.viewport_width >= MIN_WIDTH && self.viewport_height >= MIN_HEIGHT
    }

    pub fn speed(&self) -> u16 {
        SPEEDS[self.speed_index]
    }

    pub fn stamp(&mut self, pattern: Pattern, cx: i32, cy: i32) {
        for (x, y) in pattern.cells() {
            if let Some(i) = Self::index(cx + x, cy + y) {
                self.cells[i] = 1;
                self.trails[i] = 0;
            }
        }
    }

    pub fn recount(&mut self) {
        self.population = self.cells.iter().filter(|&&age| age > 0).count() as u64;
        if let Some(last) = self.history.last_mut() {
            *last = self.population;
        } else {
            self.history.push(self.population);
        }
    }

    pub fn reset_board(&mut self) {
        self.cells.fill(0);
        self.trails.fill(0);
        self.generation = 0;
        self.population = 0;
        self.births = 0;
        self.deaths = 0;
        self.clock = 0;
        self.history.clear();
    }

    pub fn home(&mut self) {
        self.camera_x = WORLD_WIDTH / 2;
        self.camera_y = WORLD_HEIGHT / 2;
        self.cursor_x = self.camera_x;
        self.cursor_y = self.camera_y;
        self.cursor_visible = false;
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.cells.len() != CELL_COUNT || self.trails.len() != CELL_COUNT {
            return Err("snapshot must contain a 192 by 128 world");
        }
        if self.speed_index >= SPEEDS.len() || self.zoom > 2 || self.clock >= 50 {
            return Err("snapshot contains invalid speed, zoom, or clock values");
        }
        if Self::index(self.camera_x, self.camera_y).is_none()
            || Self::index(self.cursor_x, self.cursor_y).is_none()
            || self.history.len() > 80
            || self.trails.iter().any(|&value| value > 12)
        {
            return Err("snapshot contains invalid coordinates or history");
        }
        if self.population != self.cells.iter().filter(|&&age| age > 0).count() as u64 {
            return Err("snapshot population does not match its cells");
        }
        Ok(())
    }
}
