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
    pub hp: u16,
    pub sprite_front_default: Option<String>,
    pub sprite_back_default: Option<String>,
    pub sprite_front_animated: Option<String>,
    pub sprite_back_animated: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct SpriteState {
    pub sprite: Option<SpriteData>,
    pub frame_index: usize,
    pub frame_tick: u64,
    pub loading: bool,
}

impl SpriteState {
    pub fn reset(&mut self) {
        self.sprite = None;
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
}

impl PlayerState {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y, steps: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    Overworld,
    Battle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleStage {
    Intro,
    Menu,
    EnemyTurn,
    Victory,
    Escape,
    Defeat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BattleState {
    pub stage: BattleStage,
    pub enemy_name: String,
    pub player_hp: u16,
    pub player_hp_max: u16,
    pub enemy_hp: u16,
    pub enemy_hp_max: u16,
    pub menu_index: usize,
    pub message: String,
    pub pending_enemy_damage: Option<u16>,
}

impl BattleState {
    pub fn new(enemy_name: String, player_hp_max: u16) -> Self {
        Self {
            stage: BattleStage::Intro,
            enemy_name,
            player_hp: player_hp_max,
            player_hp_max,
            enemy_hp: 1,
            enemy_hp_max: 1,
            menu_index: 0,
            message: "A wild Pokemon appeared!".to_string(),
            pending_enemy_damage: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub terminal_size: (u16, u16),
    pub mode: GameMode,
    pub map: MapState,
    pub player: PlayerState,
    pub player_info: Option<PokemonInfo>,
    pub enemy_info: Option<PokemonInfo>,
    pub player_sprite: SpriteState,
    pub enemy_sprite: SpriteState,
    pub battle: Option<BattleState>,
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
            mode: GameMode::Overworld,
            map,
            player: PlayerState::new(start_x, start_y),
            player_info: None,
            enemy_info: None,
            player_sprite: SpriteState::default(),
            enemy_sprite: SpriteState::default(),
            battle: None,
            message: Some("Walk through tall grass to find Pokemon.".to_string()),
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
        self.player_info.as_ref().map(|info| info.hp).unwrap_or(35)
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
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (now.as_secs() << 32) ^ now.subsec_nanos() as u64
}
