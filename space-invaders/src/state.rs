use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::DEFAULT_SEED;

// Horizontal units are base terminal columns; vertical units are half rows.
pub const WORLD_WIDTH: i32 = 80;
pub const WORLD_HEIGHT: i32 = 76;
pub const MIN_WIDTH: u16 = WORLD_WIDTH as u16;
pub const MIN_HEIGHT: u16 = WORLD_HEIGHT as u16 / 2 + 2;
pub const ALIEN_COLUMNS: u8 = 11;
pub const INITIAL_ALIEN_COUNT: usize = 5 * ALIEN_COLUMNS as usize;
pub const PLAYER_START_X: i32 = (WORLD_WIDTH - 5) / 2;
pub const PLAYER_Y: i32 = 72;
pub const SHIELD_Y: i32 = 64;
pub const ALIEN_START_X: i32 = 7;
pub const ALIEN_START_Y: i32 = 7;
pub const ALIEN_ROW_SPACING: i32 = 6;
pub const ALIEN_DROP: i32 = 4;
pub const MAX_ALIEN_SHOTS: usize = 3;
pub const FIRE_COOLDOWN: u32 = 10;
pub const RESPAWN_TICKS: u32 = 50;
pub const INVULNERABLE_TICKS: u32 = 100;
pub const WAVE_CLEAR_TICKS: u32 = 75;
pub const EXPLOSION_FRAME_TICKS: [u32; 6] = [2, 3, 3, 4, 5, 7];
pub const EXPLOSION_TICKS: u32 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Phase {
    Title,
    Playing,
    Paused,
    Respawning,
    WaveClear,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AlienKind {
    Squid,
    Crab,
    Octopus,
}

impl AlienKind {
    pub fn score(self) -> u32 {
        match self {
            Self::Squid => 30,
            Self::Crab => 20,
            Self::Octopus => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Alien {
    pub kind: AlienKind,
    pub column: u8,
    pub x: i32,
    pub y: i32,
}

impl Alien {
    pub fn bounds(&self) -> Bounds {
        Bounds::new(self.x, self.y, 5, 4)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Player {
    pub x: i32,
    pub cooldown: u32,
    pub invulnerable: u32,
}

impl Player {
    pub fn bounds(&self) -> Bounds {
        Bounds::new(self.x, PLAYER_Y, 5, 4)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Owner {
    Player,
    Alien,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Bullet {
    pub x: i32,
    pub y: i32,
    pub dy: i32,
    pub owner: Owner,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Shield {
    pub x: i32,
    pub y: i32,
    /// Six rows of seven half-cell pixels, in row-major order.
    pub pixels: Vec<bool>,
}

impl Shield {
    pub fn new(x: i32) -> Self {
        let rows = [
            "  ###  ", " ##### ", "#######", "#######", "##   ##", "##   ##",
        ];
        Self {
            x,
            y: SHIELD_Y,
            pixels: rows
                .iter()
                .flat_map(|row| row.chars().map(|c| c == '#'))
                .collect(),
        }
    }

    pub fn occupied(&self, x: i32, y: i32) -> bool {
        let (dx, dy) = (x - self.x, y - self.y);
        (0..7).contains(&dx)
            && (0..6).contains(&dy)
            && self
                .pixels
                .get((dy * 7 + dx) as usize)
                .copied()
                .unwrap_or(false)
    }

    pub fn chip(&mut self, x: i32, y: i32) {
        // Remove the struck half-cell and its immediate horizontal neighbors.
        for px in x - 1..=x + 1 {
            if self.occupied(px, y) {
                self.pixels[((y - self.y) * 7 + px - self.x) as usize] = false;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Saucer {
    pub x: i32,
    pub y: i32,
    pub direction: i32,
    pub score: u32,
}

impl Saucer {
    pub fn bounds(&self) -> Bounds {
        Bounds::new(self.x, self.y, 7, 4)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Explosion {
    pub x: i32,
    pub y: i32,
    pub age: u32,
}

impl Explosion {
    pub fn frame(&self) -> usize {
        let mut elapsed = 0;
        for (index, duration) in EXPLOSION_FRAME_TICKS.into_iter().enumerate() {
            elapsed += duration;
            if self.age < elapsed {
                return index;
            }
        }
        EXPLOSION_FRAME_TICKS.len() - 1
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, tui_dispatch::DebugState,
)]
pub struct AppState {
    #[debug(debug_fmt)]
    pub phase: Phase,
    #[debug(debug_fmt)]
    pub resume_phase: Option<Phase>,
    pub score: u32,
    pub lives: u8,
    pub wave: u32,
    pub tick: u64,
    pub seed: u64,
    pub rng_state: u64,
    #[debug(debug_fmt)]
    pub player: Player,
    #[debug(debug_fmt)]
    pub aliens: Vec<Alien>,
    #[debug(debug_fmt)]
    pub bullets: Vec<Bullet>,
    #[debug(debug_fmt)]
    pub shields: Vec<Shield>,
    #[debug(debug_fmt)]
    pub saucer: Option<Saucer>,
    #[debug(debug_fmt)]
    pub explosions: Vec<Explosion>,
    pub formation_direction: i32,
    pub formation_timer: u32,
    pub formation_frame: usize,
    pub alien_fire_timer: u32,
    pub saucer_timer: u32,
    pub phase_timer: u32,
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
            phase: Phase::Title,
            resume_phase: None,
            score: 0,
            lives: 3,
            wave: 1,
            tick: 0,
            seed,
            rng_state: seed,
            player: Player {
                x: PLAYER_START_X,
                cooldown: 0,
                invulnerable: 0,
            },
            aliens: Vec::new(),
            bullets: Vec::new(),
            shields: Vec::new(),
            saucer: None,
            explosions: Vec::new(),
            formation_direction: 1,
            formation_timer: 0,
            formation_frame: 0,
            alien_fire_timer: 0,
            saucer_timer: 1000,
            phase_timer: 0,
            viewport_width: 80,
            viewport_height: MIN_HEIGHT,
        };
        state.populate_wave();
        state
    }

    pub fn viewport_ready(&self) -> bool {
        self.viewport_width >= MIN_WIDTH && self.viewport_height >= MIN_HEIGHT
    }

    pub fn random(&mut self) -> u64 {
        // SplitMix64: deterministic, including for seed zero, and snapshot-friendly.
        self.rng_state = self.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.rng_state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    pub fn populate_wave(&mut self) {
        // Half-scale offsets from the ROM's first-rack 0x78 and subsequent
        // 0x60, 0x50, 0x48×3, 0x40×3 reference-Y entries; later entries cycle.
        let lower_start = if self.wave <= 1 {
            0
        } else {
            [12, 20, 24, 24, 24, 28, 28, 28][(self.wave as usize - 2) % 8]
        };
        self.aliens = [
            AlienKind::Squid,
            AlienKind::Crab,
            AlienKind::Crab,
            AlienKind::Octopus,
            AlienKind::Octopus,
        ]
        .into_iter()
        .enumerate()
        .flat_map(|(row, kind)| {
            (0..ALIEN_COLUMNS).map(move |column| Alien {
                kind,
                column,
                x: ALIEN_START_X + i32::from(column) * 6,
                y: ALIEN_START_Y + row as i32 * ALIEN_ROW_SPACING + lower_start,
            })
        })
        .collect();
        self.shields = [8, 27, 46, 65].into_iter().map(Shield::new).collect();
        self.bullets.clear();
        self.explosions.clear();
        self.saucer = None;
        self.player.x = PLAYER_START_X;
        self.player.cooldown = 0;
        self.formation_direction = 1;
        self.formation_timer = 0;
        self.formation_frame = 0;
        self.alien_fire_timer = 0;
        self.saucer_timer = 1000;
        self.phase_timer = 0;
    }
}
