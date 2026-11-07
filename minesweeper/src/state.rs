use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rand::seq::{IteratorRandom, SliceRandom};

pub const MIN_SIZE: usize = 5;
pub const MAX_COLUMNS: usize = 120;
pub const MAX_ROWS: usize = 120;

pub fn max_mines(width: usize, height: usize) -> usize {
    width * height * 3 / 10
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Expert,
    Custom {
        width: usize,
        height: usize,
        mines: usize,
    },
}

impl Difficulty {
    pub fn params(self) -> (usize, usize, usize) {
        match self {
            Difficulty::Beginner => (9, 9, 10),
            Difficulty::Intermediate => (16, 16, 40),
            Difficulty::Expert => (24, 20, 99),
            Difficulty::Custom {
                width,
                height,
                mines,
            } => {
                let width = width.clamp(MIN_SIZE, MAX_COLUMNS);
                let height = height.clamp(MIN_SIZE, MAX_ROWS);
                (width, height, mines.clamp(1, max_mines(width, height)))
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Beginner => "Beginner",
            Difficulty::Intermediate => "Intermediate",
            Difficulty::Expert => "Expert",
            Difficulty::Custom { .. } => "Custom",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomForm {
    pub values: [String; 3],
    pub field: usize,
    pub replace: bool,
    pub error: Option<String>,
}

impl CustomForm {
    pub fn new(state: &AppState) -> Self {
        Self {
            values: [
                state.width.to_string(),
                state.height.to_string(),
                state.mine_count.to_string(),
            ],
            field: 0,
            replace: true,
            error: None,
        }
    }

    pub fn max_digits(&self) -> usize {
        if self.field == 2 {
            4
        } else {
            3
        }
    }

    pub fn mine_limit(&self) -> usize {
        let width = self.values[0]
            .parse::<usize>()
            .unwrap_or(MIN_SIZE)
            .clamp(MIN_SIZE, MAX_COLUMNS);
        let height = self.values[1]
            .parse::<usize>()
            .unwrap_or(MIN_SIZE)
            .clamp(MIN_SIZE, MAX_ROWS);
        max_mines(width, height)
    }

    pub fn difficulty(&self) -> Result<Difficulty, (usize, &'static str)> {
        let width = self.values[0]
            .parse::<usize>()
            .ok()
            .filter(|v| (MIN_SIZE..=MAX_COLUMNS).contains(v))
            .ok_or((0, "Columns: 5–120"))?;
        let height = self.values[1]
            .parse::<usize>()
            .ok()
            .filter(|v| (MIN_SIZE..=MAX_ROWS).contains(v))
            .ok_or((1, "Rows: 5–120"))?;
        let mines = self.values[2]
            .parse::<usize>()
            .ok()
            .filter(|v| (1..=max_mines(width, height)).contains(v))
            .ok_or((2, "Mines: 1–30% of cells"))?;
        Ok(Difficulty::Custom {
            width,
            height,
            mines,
        })
    }
}

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Cell {
    pub mine: bool,
    pub revealed: bool,
    pub flagged: bool,
    pub adjacent: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum GameState {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema, tui_dispatch::DebugState)]
pub struct AppState {
    #[debug(skip)]
    pub grid: Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    #[debug(debug_fmt)]
    pub game_state: GameState,
    #[debug(debug_fmt)]
    pub difficulty: Difficulty,
    pub mine_count: usize,
    pub flags_placed: usize,
    pub cells_revealed: usize,
    pub total_safe: usize,
    #[debug(skip)]
    pub custom_form: Option<CustomForm>,
}

impl AppState {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(MIN_SIZE..=MAX_COLUMNS).contains(&self.width)
            || !(MIN_SIZE..=MAX_ROWS).contains(&self.height)
            || self.grid.len() != self.height
            || self.grid.iter().any(|row| row.len() != self.width)
            || self.cursor_x >= self.width
            || self.cursor_y >= self.height
            || self.mine_count == 0
            || self.mine_count > max_mines(self.width, self.height)
            || self.grid.iter().flatten().filter(|cell| cell.mine).count() != self.mine_count
            || self.total_safe != self.width * self.height - self.mine_count
            || self.cells_revealed
                != self
                    .grid
                    .iter()
                    .flatten()
                    .filter(|cell| cell.revealed && !cell.mine)
                    .count()
            || self.flags_placed
                != self
                    .grid
                    .iter()
                    .flatten()
                    .filter(|cell| cell.flagged)
                    .count()
            || self.grid.iter().flatten().any(|cell| cell.adjacent > 8)
            || self
                .custom_form
                .as_ref()
                .is_some_and(|form| form.field >= 3)
        {
            return Err("snapshot contains an invalid board");
        }
        Ok(())
    }

    pub fn protect_first_reveal(&mut self, x: usize, y: usize) {
        if self.cells_revealed != 0
            || !self
                .grid
                .get(y)
                .and_then(|row| row.get(x))
                .is_some_and(|cell| cell.mine)
        {
            return;
        }

        // Relocate instead of rerolling: the opening is safe for every board,
        // with the same mine count and any flags placed before the first reveal.
        let destination = self
            .grid
            .iter()
            .enumerate()
            .flat_map(|(y, row)| {
                row.iter()
                    .enumerate()
                    .filter_map(move |(x, cell)| (!cell.mine).then_some((x, y)))
            })
            .choose(&mut rand::rng());
        let Some((safe_x, safe_y)) = destination else {
            return;
        };
        self.grid[y][x].mine = false;
        self.grid[safe_y][safe_x].mine = true;
        update_adjacent_counts(&mut self.grid, self.width, self.height);
    }

    pub fn new(difficulty: Difficulty) -> Self {
        let (width, height, mines) = difficulty.params();
        let mut grid = vec![vec![Cell::default(); width]; height];

        // Place mines randomly
        let mut positions: Vec<(usize, usize)> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .collect();
        positions.shuffle(&mut rand::rng());

        for &(x, y) in positions.iter().take(mines) {
            grid[y][x].mine = true;
        }

        update_adjacent_counts(&mut grid, width, height);

        Self {
            grid,
            width,
            height,
            cursor_x: width / 2,
            cursor_y: height / 2,
            game_state: GameState::Playing,
            difficulty,
            mine_count: mines,
            flags_placed: 0,
            cells_revealed: 0,
            total_safe: width * height - mines,
            custom_form: None,
        }
    }
}

fn update_adjacent_counts(grid: &mut [Vec<Cell>], width: usize, height: usize) {
    for y in 0..height {
        for x in 0..width {
            grid[y][x].adjacent = if grid[y][x].mine {
                0
            } else {
                count_adjacent_mines(grid, x, y, width, height)
            };
        }
    }
}

fn count_adjacent_mines(grid: &[Vec<Cell>], x: usize, y: usize, width: usize, height: usize) -> u8 {
    neighbors(x, y, width, height)
        .iter()
        .filter(|&&(nx, ny)| grid[ny][nx].mine)
        .count() as u8
}

/// Return valid neighbor coordinates within the grid bounds.
pub fn neighbors(x: usize, y: usize, width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::with_capacity(8);
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                result.push((nx as usize, ny as usize));
            }
        }
    }
    result
}
