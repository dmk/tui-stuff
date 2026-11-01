use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Pattern {
    Garden,
    GosperGun,
    Pulsar,
    Glider,
    RPentomino,
    Acorn,
}

impl Pattern {
    pub const ALL: [Self; 6] = [
        Self::Garden,
        Self::GosperGun,
        Self::Pulsar,
        Self::Glider,
        Self::RPentomino,
        Self::Acorn,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Garden => "Midnight garden",
            Self::GosperGun => "Gosper glider gun",
            Self::Pulsar => "Pulsar",
            Self::Glider => "Glider",
            Self::RPentomino => "R-pentomino",
            Self::Acorn => "Acorn",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Garden => "A little ecosystem.\nOscillators, travelers,\nand islands of calm.",
            Self::GosperGun => {
                "Order that creates motion.\nA new glider emerges\nevery 30 generations."
            }
            Self::Pulsar => "A three-beat heartbeat.\nSymmetry, briefly broken,\nthen restored.",
            Self::Glider => "Five cells on a journey.\nOne diagonal step every\nfour generations.",
            Self::RPentomino => {
                "Five cells. A surprisingly\nlong story. Let it run\nand watch it unfold."
            }
            Self::Acorn => "Seven cells, planted.\nA small beginning for\na sprawling world.",
        }
    }

    /// Coordinates are relative to the center, for both loading and stamping.
    pub fn cells(self) -> Vec<(i32, i32)> {
        if self == Self::Garden {
            let mut cells = Self::Pulsar.cells();
            for (cx, cy) in [(-16, -9), (15, 8), (16, -10), (-17, 8)] {
                cells.extend(
                    Self::Glider
                        .cells()
                        .into_iter()
                        .map(|(x, y)| (x + cx, y + cy)),
                );
            }
            for (cx, cy) in [(-11, -10), (10, 10), (-12, 10), (11, -10)] {
                cells.extend([(cx, cy), (cx + 1, cy), (cx, cy + 1), (cx + 1, cy + 1)]);
            }
            for (cx, cy) in [(-15, 0), (15, 0)] {
                cells.extend([(cx - 1, cy), (cx, cy), (cx + 1, cy)]);
            }
            return cells;
        }
        let rows: &[&str] = match self {
            Self::Pulsar => &[
                "..OOO...OOO..",
                ".............",
                "O....O.O....O",
                "O....O.O....O",
                "O....O.O....O",
                "..OOO...OOO..",
                ".............",
                "..OOO...OOO..",
                "O....O.O....O",
                "O....O.O....O",
                "O....O.O....O",
                ".............",
                "..OOO...OOO..",
            ],
            Self::GosperGun => &[
                "........................O...........",
                "......................O.O...........",
                "............OO......OO............OO",
                "...........O...O....OO............OO",
                "OO........O.....O...OO..............",
                "OO........O...O.OO....O.O...........",
                "..........O.....O.......O...........",
                "...........O...O....................",
                "............OO......................",
            ],
            Self::Glider => &[".O.", "..O", "OOO"],
            Self::RPentomino => &[".OO", "OO.", ".O."],
            Self::Acorn => &[".O.....", "...O...", "OO..OOO"],
            Self::Garden => unreachable!(),
        };
        let width = rows.iter().map(|row| row.len()).max().unwrap_or(0) as i32;
        let height = rows.len() as i32;
        rows.iter()
            .enumerate()
            .flat_map(|(y, row)| {
                row.bytes().enumerate().filter_map(move |(x, cell)| {
                    (cell == b'O').then_some((x as i32 - width / 2, y as i32 - height / 2))
                })
            })
            .collect()
    }
}
