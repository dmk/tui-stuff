use serde::{Deserialize, Serialize};
use tui_dispatch_debug::debug::{ron_string, DebugSection, DebugState};

use crate::sprite::SpriteData;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpriteTarget {
    Player,
    Enemy,
}

impl SpriteTarget {
    pub fn label(self) -> &'static str {
        match self {
            SpriteTarget::Player => "player",
            SpriteTarget::Enemy => "enemy",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokemonInfo {
    pub name: String,
    #[serde(default = "default_base_experience")]
    pub base_experience: u16,
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub sp_attack: u16,
    pub sp_defense: u16,
    pub speed: u16,
    pub sprite_front_default: Option<String>,
    pub sprite_back_default: Option<String>,
    pub sprite_front_animated: Option<String>,
    pub sprite_back_animated: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Potion,
    SuperPotion,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Potion => "Potion",
            ItemKind::SuperPotion => "Super Potion",
        }
    }

    pub fn heal_amount(self) -> u16 {
        match self {
            ItemKind::Potion => 20,
            ItemKind::SuperPotion => 50,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    pub kind: ItemKind,
    pub qty: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct SpriteState {
    pub sprite: Option<SpriteData>,
    #[serde(default)]
    pub sprite_flipped: Option<SpriteData>,
    pub frame_index: usize,
    pub frame_tick: u64,
    pub loading: bool,
}

impl SpriteState {
    pub fn reset(&mut self) {
        self.sprite = None;
        self.sprite_flipped = None;
        self.frame_index = 0;
        self.frame_tick = 0;
        self.loading = false;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Grass,
    Path,
    Sand,
    Wall,
    Water,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapState {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub tiles: Vec<Tile>,
}

impl MapState {
    pub fn new() -> Self {
        Self::from_str(
            "LAKESIDE ROUTE",
            r#"
##################################################
#gggggggggggggggggggggggggggggggggggggggggggggggg#
#ggggggggggggssssssssssssgggggggggggggggggggggggg#
#ggggggggggsswwwwwwwwwwwwssgggggggggggggggggggggg#
#gggggggggsswwwwwwwwwwwwwwssggggggggggggggggggggg#
#ggggggggsswwwwwwwwwwwwwwwwssgggggggggggggggggggg#
#gggggggswwwwwwwwwwwwwwwwwwwsgggggggggggggggggggg#
#gggggggsswwwwwwwwwwwwwwwwwssgggggggggggggggggggg#
#ggggggggsswwwwwwwwwwwwwwwssggggggggggggggggggggg#
#gggggggggsswwwwwwwwwwwwwssgggggggggggggggggggggg#
#ggggggggggssssssssssssssssggggggggggggggggggggg#
#gggggggggggggggggggggggggggggggggggggggggggggggg#
#rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr#
#gggggggggggggggggggggggggggggggggggggggggggggggg#
#gggggggggggggggggggggggggggggggggggggggggggggggg#
#ggggggggg###ggggggggggggggggggggggg###gggggggggg#
#ggggggggg#r#ggggggggggggggggggggggg#r#gggggggggg#
#ggggggggg#r#ggggggggggggggggggggggg#r#gggggggggg#
#ggggggggg#r#ggggggggggggggggggggggg#r#gggggggggg#
#ggggggggg#rrrrrrrrrrrrrrrrrrrrrrrrrrr#gggggggggg#
#ggggggggg#######################r####gggggggggg#
#ggggggggggggggggggggggggggggggggrgggggggggggggg#
#ggggggggggggggggggggggggggggggggrgggggggggggggg#
##################################################
"#,
        )
    }

    pub fn from_str(name: &str, map_str: &str) -> Self {
        let lines: Vec<&str> = map_str
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        let height = lines.len();
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

        let mut tiles = Vec::with_capacity(width * height);
        for line in &lines {
            let chars: Vec<char> = line.chars().collect();
            for x in 0..width {
                let ch = chars.get(x).copied().unwrap_or('g');
                tiles.push(Self::char_to_tile(ch));
            }
        }

        Self {
            name: name.to_string(),
            width: width as u16,
            height: height as u16,
            tiles,
        }
    }

    fn char_to_tile(ch: char) -> Tile {
        match ch {
            'g' | 'G' => Tile::Grass,
            'r' | 'R' | 'p' | 'P' => Tile::Path,
            's' | 'S' => Tile::Sand,
            'w' | 'W' => Tile::Water,
            '#' | 'x' | 'X' => Tile::Wall,
            _ => Tile::Grass,
        }
    }

    pub fn start_pos(&self) -> (u16, u16) {
        // Find first path tile, or default to (2, height/2)
        for y in 0..self.height {
            for x in 0..self.width {
                if matches!(self.tile(x, y), Tile::Path) {
                    return (x, y);
                }
            }
        }
        (2, self.height / 2)
    }

    pub fn tile(&self, x: u16, y: u16) -> Tile {
        if x >= self.width || y >= self.height {
            return Tile::Wall;
        }
        let idx = self.index(x, y);
        self.tiles.get(idx).copied().unwrap_or(Tile::Wall)
    }

    pub fn is_walkable(&self, x: u16, y: u16) -> bool {
        !matches!(self.tile(x, y), Tile::Wall | Tile::Water)
    }

    pub fn is_grass(&self, x: u16, y: u16) -> bool {
        matches!(self.tile(x, y), Tile::Grass)
    }

    fn index(&self, x: u16, y: u16) -> usize {
        (y as usize * self.width as usize) + x as usize
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub x: u16,
    pub y: u16,
    pub steps: u64,
    pub facing: Direction,
}

impl PlayerState {
    pub fn new(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            steps: 0,
            facing: Direction::Down,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    MainMenu,
    PokemonSelect,
    Overworld,
    Battle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuState {
    pub selected: usize,
    pub has_save: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PokemonSelectState {
    pub starters: Vec<String>,
    pub selected: usize,
    pub preview_info: Option<PokemonInfo>,
    pub preview_sprite: SpriteState,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PauseMenuState {
    pub is_open: bool,
    pub selected: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleStage {
    Intro,
    Menu,
    ItemMenu,
    EnemyTurn,
    Victory,
    Escape,
    Defeat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BattleState {
    pub stage: BattleStage,
    pub enemy_name: String,
    #[serde(default = "default_enemy_level")]
    pub enemy_level: u8,
    pub player_hp: u16,
    pub player_hp_max: u16,
    pub enemy_hp: u16,
    pub enemy_hp_max: u16,
    pub menu_index: usize,
    #[serde(default)]
    pub item_index: usize,
    pub message: String,
    pub pending_enemy_damage: Option<u16>,
}

impl BattleState {
    pub fn new(enemy_name: String, enemy_level: u8, player_hp_max: u16, player_hp: u16) -> Self {
        Self {
            stage: BattleStage::Intro,
            enemy_name,
            enemy_level,
            player_hp: player_hp.min(player_hp_max),
            player_hp_max,
            enemy_hp: 1,
            enemy_hp_max: 1,
            menu_index: 0,
            item_index: 0,
            message: "A wild Pokemon appeared!".to_string(),
            pending_enemy_damage: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppState {
    pub terminal_size: (u16, u16),
    pub mode: GameMode,
    pub map: MapState,
    pub player: PlayerState,
    pub player_info: Option<PokemonInfo>,
    #[serde(default = "default_player_level")]
    pub player_level: u8,
    #[serde(default = "default_player_exp")]
    pub player_exp: u32,
    #[serde(default = "default_player_hp")]
    pub player_hp: u16,
    #[serde(default = "default_inventory")]
    pub inventory: Vec<ItemStack>,
    pub enemy_info: Option<PokemonInfo>,
    pub player_sprite: SpriteState,
    pub enemy_sprite: SpriteState,
    pub battle: Option<BattleState>,
    pub menu: Option<MenuState>,
    pub pokemon_select: Option<PokemonSelectState>,
    pub pause_menu: PauseMenuState,
    pub message: Option<String>,
    pub steps_since_encounter: u16,
    pub rng_seed: u64,
    pub tick: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let map = MapState::new();
        let (start_x, start_y) = map.start_pos();
        Self {
            terminal_size: (80, 24),
            mode: GameMode::MainMenu,
            map,
            player: PlayerState::new(start_x, start_y),
            player_info: None,
            player_level: default_player_level(),
            player_exp: default_player_exp(),
            player_hp: default_player_hp(),
            inventory: default_inventory(),
            enemy_info: None,
            player_sprite: SpriteState::default(),
            enemy_sprite: SpriteState::default(),
            battle: None,
            menu: Some(MenuState {
                selected: 0,
                has_save: false,
            }),
            pokemon_select: None,
            pause_menu: PauseMenuState::default(),
            message: None,
            steps_since_encounter: 0,
            rng_seed: seed_from_time(),
            tick: 0,
        }
    }

    pub fn player_name(&self) -> String {
        self.player_info
            .as_ref()
            .map(|info| info.name.clone())
            .unwrap_or_else(|| "partner".to_string())
    }

    pub fn player_max_hp(&self) -> u16 {
        self.player_info
            .as_ref()
            .map(|info| calc_hp(info.hp, self.player_level))
            .unwrap_or(35)
    }

    pub fn exp_to_next_level(&self) -> u32 {
        if self.player_level >= MAX_LEVEL {
            return 0;
        }
        let next = exp_for_level(self.player_level.saturating_add(1));
        let current = exp_for_level(self.player_level);
        next.saturating_sub(current)
    }

    pub fn exp_progress(&self) -> u32 {
        let current = exp_for_level(self.player_level);
        self.player_exp.saturating_sub(current)
    }
}

impl DebugState for AppState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        let mut sections = vec![
            DebugSection::new("Mode")
                .entry("mode", ron_string(&self.mode))
                .entry("message", ron_string(&self.message)),
            DebugSection::new("Player")
                .entry("x", ron_string(&self.player.x))
                .entry("y", ron_string(&self.player.y))
                .entry("steps", ron_string(&self.player.steps)),
        ];

        if let Some(battle) = &self.battle {
            sections.push(
                DebugSection::new("Battle")
                    .entry("stage", ron_string(&battle.stage))
                    .entry("enemy", ron_string(&battle.enemy_name))
                    .entry("player_hp", ron_string(&battle.player_hp))
                    .entry("enemy_hp", ron_string(&battle.enemy_hp)),
            );
        }

        sections
    }
}

fn seed_from_time() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (now.as_secs() << 32) ^ now.subsec_nanos() as u64
}

pub const MAX_LEVEL: u8 = 100;

pub fn exp_for_level(level: u8) -> u32 {
    let level = level.max(1) as u32;
    level.pow(3)
}

pub fn calc_hp(base: u16, level: u8) -> u16 {
    let level = level.max(1) as u16;
    (((2 * base + 31) * level) / 100) + level + 10
}

pub fn calc_stat(base: u16, level: u8) -> u16 {
    let level = level.max(1) as u16;
    (((2 * base + 31) * level) / 100) + 5
}

fn default_base_experience() -> u16 {
    60
}

fn default_enemy_level() -> u8 {
    5
}

fn default_player_level() -> u8 {
    5
}

fn default_player_exp() -> u32 {
    exp_for_level(default_player_level())
}

fn default_player_hp() -> u16 {
    0
}

fn default_inventory() -> Vec<ItemStack> {
    vec![
        ItemStack {
            kind: ItemKind::Potion,
            qty: 3,
        },
        ItemStack {
            kind: ItemKind::SuperPotion,
            qty: 1,
        },
    ]
}
