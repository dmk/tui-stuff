use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileKind {
    Grass,
    Trail,
    Sand,
    Floor,
    Wall,
    Water,
    Custom(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapSize {
    pub width: u16,
    pub height: u16,
}

impl MapSize {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    pub fn tile_count(self) -> usize {
        self.width as usize * self.height as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreError {
    TileCountMismatch { expected: usize, actual: usize },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::TileCountMismatch { expected, actual } => {
                write!(f, "tile count mismatch: expected {}, got {}", expected, actual)
            }
        }
    }
}

impl std::error::Error for CoreError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapGrid {
    pub name: String,
    pub size: MapSize,
    pub tiles: Vec<TileKind>,
}

impl MapGrid {
    pub fn new(name: impl Into<String>, size: MapSize, tiles: Vec<TileKind>) -> Result<Self, CoreError> {
        let expected = size.tile_count();
        let actual = tiles.len();
        if expected != actual {
            return Err(CoreError::TileCountMismatch { expected, actual });
        }
        Ok(Self {
            name: name.into(),
            size,
            tiles,
        })
    }

    pub fn filled(name: impl Into<String>, size: MapSize, tile: TileKind) -> Self {
        Self {
            name: name.into(),
            size,
            tiles: vec![tile; size.tile_count()],
        }
    }

    pub fn width(&self) -> u16 {
        self.size.width
    }

    pub fn height(&self) -> u16 {
        self.size.height
    }

    pub fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.size.width || y >= self.size.height {
            return None;
        }
        Some(y as usize * self.size.width as usize + x as usize)
    }

    pub fn tile_at(&self, x: u16, y: u16) -> Option<TileKind> {
        let idx = self.index(x, y)?;
        self.tiles.get(idx).copied()
    }

    pub fn tile_kind(&self, x: u16, y: u16) -> TileKind {
        self.tile_at(x, y).unwrap_or(TileKind::Wall)
    }
}

pub trait MapRead {
    fn map_size(&self) -> MapSize;
    fn tile_kind(&self, x: u16, y: u16) -> TileKind;
}

impl MapRead for MapGrid {
    fn map_size(&self) -> MapSize {
        self.size
    }

    fn tile_kind(&self, x: u16, y: u16) -> TileKind {
        self.tile_kind(x, y)
    }
}

pub fn viewport_centered(
    focus_x: u16,
    focus_y: u16,
    map: MapSize,
    view_cols: u16,
    view_rows: u16,
) -> (u16, u16) {
    if map.width == 0 || map.height == 0 || view_cols == 0 || view_rows == 0 {
        return (0, 0);
    }

    let half_cols = view_cols / 2;
    let half_rows = view_rows / 2;
    let max_x = map.width.saturating_sub(view_cols);
    let max_y = map.height.saturating_sub(view_rows);
    let start_x = focus_x.saturating_sub(half_cols).min(max_x);
    let start_y = focus_y.saturating_sub(half_rows).min(max_y);
    (start_x, start_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_grid_index_and_bounds() {
        let map = MapGrid::filled("demo", MapSize::new(3, 2), TileKind::Grass);
        assert_eq!(map.index(0, 0), Some(0));
        assert_eq!(map.index(2, 1), Some(5));
        assert_eq!(map.index(3, 0), None);
        assert_eq!(map.index(0, 2), None);
    }

    #[test]
    fn map_grid_tile_kind_out_of_bounds_defaults_to_wall() {
        let map = MapGrid::filled("demo", MapSize::new(2, 2), TileKind::Grass);
        assert_eq!(map.tile_kind(8, 8), TileKind::Wall);
    }

    #[test]
    fn viewport_clamps_to_map() {
        let map = MapSize::new(20, 10);
        assert_eq!(viewport_centered(0, 0, map, 8, 6), (0, 0));
        assert_eq!(viewport_centered(19, 9, map, 8, 6), (12, 4));
        assert_eq!(viewport_centered(10, 5, map, 8, 6), (6, 2));
    }
}
