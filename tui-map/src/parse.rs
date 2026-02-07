use std::collections::HashMap;
use std::fmt;

use crate::core::{MapGrid, MapSize, TileKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrimMode {
    PreserveRightWhitespace,
    TrimBoth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseOptions {
    pub trim_mode: TrimMode,
    pub default_char: char,
    pub default_tile: TileKind,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            trim_mode: TrimMode::TrimBoth,
            default_char: ' ',
            default_tile: TileKind::Wall,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Legend {
    map: HashMap<char, TileKind>,
}

impl Legend {
    pub fn builder() -> LegendBuilder {
        LegendBuilder::default()
    }

    pub fn tile_for(&self, ch: char) -> Option<TileKind> {
        self.map.get(&ch).copied()
    }
}

#[derive(Clone, Debug, Default)]
pub struct LegendBuilder {
    entries: HashMap<char, TileKind>,
}

impl LegendBuilder {
    pub fn entry(mut self, ch: char, tile: TileKind) -> Self {
        self.entries.insert(ch, tile);
        self
    }

    pub fn build(self) -> Result<Legend, ParseError> {
        if self.entries.is_empty() {
            return Err(ParseError::EmptyLegend);
        }

        if self.entries.keys().any(|ch| *ch == '\0') {
            return Err(ParseError::InvalidLegendChar);
        }

        Ok(Legend { map: self.entries })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    EmptyLegend,
    InvalidLegendChar,
    UnknownLegendKey(char),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptyLegend => write!(f, "legend must contain at least one entry"),
            ParseError::InvalidLegendChar => write!(f, "legend contains an invalid character"),
            ParseError::UnknownLegendKey(ch) => {
                write!(f, "map contains unknown legend key: {:?}", ch)
            }
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse_char_grid(
    map_name: &str,
    map_text: &str,
    legend: &Legend,
    options: &ParseOptions,
) -> Result<MapGrid, ParseError> {
    let lines: Vec<String> = map_text
        .lines()
        .map(|line| match options.trim_mode {
            TrimMode::TrimBoth => line.trim().to_string(),
            TrimMode::PreserveRightWhitespace => line.trim_end().to_string(),
        })
        .filter(|line| !line.trim().is_empty())
        .collect();

    let height = lines.len();
    let width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);

    let mut tiles = Vec::with_capacity(width * height);
    for line in &lines {
        let chars: Vec<char> = line.chars().collect();
        for x in 0..width {
            let ch = chars.get(x).copied().unwrap_or(options.default_char);
            let tile = if let Some(tile) = legend.tile_for(ch) {
                tile
            } else if ch == options.default_char {
                options.default_tile
            } else {
                return Err(ParseError::UnknownLegendKey(ch));
            };
            tiles.push(tile);
        }
    }

    let grid = MapGrid::new(map_name.to_string(), MapSize::new(width as u16, height as u16), tiles)
        .expect("parser precomputes exact tile capacity");
    Ok(grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_legend() -> Legend {
        Legend::builder()
            .entry('g', TileKind::Grass)
            .entry('#', TileKind::Wall)
            .build()
            .expect("legend")
    }

    #[test]
    fn parse_ragged_lines_with_default_fill() {
        let legend = sample_legend();
        let map = parse_char_grid(
            "demo",
            "\n##\n#g\n#\n",
            &legend,
            &ParseOptions {
                trim_mode: TrimMode::TrimBoth,
                default_char: 'g',
                default_tile: TileKind::Grass,
            },
        )
        .expect("map");

        assert_eq!(map.size, MapSize::new(2, 3));
        assert_eq!(map.tile_kind(1, 2), TileKind::Grass);
    }

    #[test]
    fn parse_errors_on_unknown_legend_key() {
        let legend = sample_legend();
        let err = parse_char_grid(
            "demo",
            "g?",
            &legend,
            &ParseOptions {
                trim_mode: TrimMode::TrimBoth,
                default_char: 'g',
                default_tile: TileKind::Grass,
            },
        )
        .expect_err("should fail");

        assert_eq!(err, ParseError::UnknownLegendKey('?'));
    }

    #[test]
    fn trim_mode_parity() {
        let legend = Legend::builder()
            .entry(' ', TileKind::Floor)
            .entry('g', TileKind::Grass)
            .build()
            .expect("legend");

        let trimmed = parse_char_grid(
            "trim",
            "   g   ",
            &legend,
            &ParseOptions {
                trim_mode: TrimMode::TrimBoth,
                default_char: ' ',
                default_tile: TileKind::Floor,
            },
        )
        .expect("trimmed map");

        let keep_left = parse_char_grid(
            "keep-left",
            "   g   ",
            &legend,
            &ParseOptions {
                trim_mode: TrimMode::PreserveRightWhitespace,
                default_char: ' ',
                default_tile: TileKind::Floor,
            },
        )
        .expect("left-preserved map");

        assert_eq!(trimmed.size.width, 1);
        assert_eq!(keep_left.size.width, 4);
    }

    #[test]
    fn legend_builder_rejects_empty() {
        let err = Legend::builder().build().expect_err("should fail");
        assert_eq!(err, ParseError::EmptyLegend);
    }
}
