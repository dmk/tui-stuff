use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tui_map::core::{MapGrid, MapRead, MapSize, TileKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum GameMode {
    Boot,
    Exploration,
    Pause,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DangerMode {
    SoundHunter,
    ImminentCollapse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum RuntimeAnchorKind {
    PlayerStart,
    Exit,
    Beacon,
    Relic,
    Switch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeAnchor {
    pub kind: RuntimeAnchorKind,
    pub x: u16,
    pub y: u16,
    pub tag: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Tile {
    Floor,
    Wall,
    Water,
    Trail,
    Grass,
}

impl Tile {
    pub fn to_tile_kind(self) -> TileKind {
        match self {
            Tile::Floor => TileKind::Floor,
            Tile::Wall => TileKind::Wall,
            Tile::Water => TileKind::Water,
            Tile::Trail => TileKind::Trail,
            Tile::Grass => TileKind::Grass,
        }
    }

    pub fn from_tile_kind(kind: TileKind) -> Self {
        match kind {
            TileKind::Floor => Tile::Floor,
            TileKind::Wall => Tile::Wall,
            TileKind::Water => Tile::Water,
            TileKind::Trail => Tile::Trail,
            TileKind::Grass => Tile::Grass,
            TileKind::Sand => Tile::Floor,
            TileKind::Custom(_) => Tile::Floor,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapState {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub tiles: Vec<Tile>,
}

impl MapState {
    pub fn from_grid(grid: MapGrid) -> Self {
        Self {
            name: grid.name,
            width: grid.size.width,
            height: grid.size.height,
            tiles: grid.tiles.into_iter().map(Tile::from_tile_kind).collect(),
        }
    }

    pub fn filled(name: impl Into<String>, size: MapSize, tile: TileKind) -> Self {
        Self::from_grid(MapGrid::filled(name, size, tile))
    }

    pub fn tile(&self, x: u16, y: u16) -> Tile {
        if x >= self.width || y >= self.height {
            return Tile::Wall;
        }
        let idx = self.index(x, y);
        self.tiles.get(idx).copied().unwrap_or(Tile::Wall)
    }

    pub fn is_walkable(&self, x: u16, y: u16) -> bool {
        matches!(self.tile(x, y), Tile::Floor | Tile::Trail | Tile::Grass)
    }

    pub fn is_light_blocker(&self, x: u16, y: u16) -> bool {
        matches!(self.tile(x, y), Tile::Wall)
    }

    fn index(&self, x: u16, y: u16) -> usize {
        (y as usize * self.width as usize) + x as usize
    }
}

impl MapRead for MapState {
    fn map_size(&self) -> MapSize {
        MapSize::new(self.width, self.height)
    }

    fn tile_kind(&self, x: u16, y: u16) -> TileKind {
        self.tile(x, y).to_tile_kind()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TrailState {
    pub width: u16,
    pub height: u16,
    pub charges: Vec<u16>,
}

impl TrailState {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            charges: vec![0; width as usize * height as usize],
        }
    }

    pub fn deposit(&mut self, x: u16, y: u16, amount: u16) {
        if let Some(idx) = self.index(x, y) {
            self.charges[idx] = self.charges[idx].saturating_add(amount);
        }
    }

    pub fn take(&mut self, x: u16, y: u16) -> u16 {
        let Some(idx) = self.index(x, y) else {
            return 0;
        };
        let amount = self.charges[idx];
        self.charges[idx] = 0;
        amount
    }

    pub fn charge_at(&self, x: u16, y: u16) -> u16 {
        let Some(idx) = self.index(x, y) else {
            return 0;
        };
        self.charges[idx]
    }

    pub fn clear(&mut self) {
        self.charges.fill(0);
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerState {
    pub x: u16,
    pub y: u16,
    pub light_current: u16,
    pub light_max: u16,
    pub steps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedFloor {
    pub map: MapState,
    pub anchors: Vec<RuntimeAnchor>,
    pub danger_mode: DangerMode,
    pub generator_id: String,
    pub generator_version: u32,
    pub seed: u64,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AppState {
    pub mode: GameMode,
    pub floor_index: u32,
    pub seed: u64,
    pub map: MapState,
    pub player: PlayerState,
    pub trail: TrailState,
    pub danger_mode: DangerMode,
    pub anchors: Vec<RuntimeAnchor>,
    pub last_status: Option<String>,
}

impl AppState {
    pub fn new(seed: u64) -> Self {
        let map = MapState::filled("bootstrap", MapSize::new(3, 3), TileKind::Wall);
        Self {
            mode: GameMode::Boot,
            floor_index: 0,
            seed,
            map,
            player: PlayerState {
                x: 1,
                y: 1,
                light_current: 120,
                light_max: 120,
                steps: 0,
            },
            trail: TrailState::new(3, 3),
            danger_mode: DangerMode::SoundHunter,
            anchors: Vec::new(),
            last_status: None,
        }
    }

    pub fn apply_generated_floor(&mut self, floor: GeneratedFloor) {
        self.map = floor.map;
        self.trail = TrailState::new(self.map.width, self.map.height);
        self.anchors = floor.anchors;
        self.danger_mode = floor.danger_mode;

        if let Some((x, y)) = self.anchor_pos(RuntimeAnchorKind::PlayerStart) {
            self.player.x = x;
            self.player.y = y;
        } else {
            self.player.x = 1.min(self.map.width.saturating_sub(1));
            self.player.y = 1.min(self.map.height.saturating_sub(1));
        }

        self.mode = GameMode::Exploration;
        self.last_status = Some(format!(
            "Floor {} ready ({:?})",
            self.floor_index + 1,
            self.danger_mode
        ));
    }

    pub fn player_pos(&self) -> (u16, u16) {
        (self.player.x, self.player.y)
    }

    pub fn exit_pos(&self) -> Option<(u16, u16)> {
        self.anchor_pos(RuntimeAnchorKind::Exit)
    }

    pub fn anchor_pos(&self, kind: RuntimeAnchorKind) -> Option<(u16, u16)> {
        self.anchors
            .iter()
            .find(|anchor| anchor.kind == kind)
            .map(|anchor| (anchor.x, anchor.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trail_deposit_and_take() {
        let mut trail = TrailState::new(4, 4);
        trail.deposit(2, 2, 3);
        assert_eq!(trail.take(2, 2), 3);
        assert_eq!(trail.take(2, 2), 0);
    }

    #[test]
    fn out_of_bounds_tile_is_wall() {
        let map = MapState::filled("test", MapSize::new(4, 4), TileKind::Floor);
        assert_eq!(map.tile(99, 99), Tile::Wall);
    }
}
