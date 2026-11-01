use crate::state::{WORLD_HEIGHT, WORLD_WIDTH};
use artbox::sprites::{Sprite, SpriteLayer, SpriteRendered, SpriteSelection, SpriteVariant};
use artbox::{Color, ColorStop, Fill, LinearGradient, RadialGradient};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SpriteSizing {
    /// Keep the original 5×2 silhouettes at every terminal size.
    Compact,
    /// Choose the largest hand-drawn variant that fits.
    #[default]
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteSize {
    Small,
    Medium,
    Large,
}

impl SpriteSize {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    pub fn id(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn alien_bounds(self) -> (u16, u16) {
        match self {
            Self::Small => (5, 2),
            Self::Medium => (7, 3),
            Self::Large => (11, 4),
        }
    }

    pub fn board_bounds(self) -> (u16, u16) {
        let (width, height) = self.alien_bounds();
        (
            (WORLD_WIDTH as u16 * width).div_ceil(5),
            (WORLD_HEIGHT as u16 * height).div_ceil(4),
        )
    }

    pub fn fit(width: u16, height: u16) -> Option<Self> {
        Self::ALL.into_iter().rev().find(|size| {
            let (w, h) = size.board_bounds();
            width >= w && height >= h + 2
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteId {
    Squid,
    Crab,
    Octopus,
    Player,
    Saucer,
    Explosion,
}

pub const MAGENTA: (u8, u8, u8) = (235, 120, 255);
pub const CYAN: (u8, u8, u8) = (90, 225, 255);
pub const GREEN: (u8, u8, u8) = (110, 245, 145);
pub const WHITE: (u8, u8, u8) = (240, 245, 255);
pub const RED: (u8, u8, u8) = (255, 100, 115);
pub const YELLOW: (u8, u8, u8) = (255, 225, 110);

impl SpriteId {
    pub const ALL: [Self; 6] = [
        Self::Squid,
        Self::Crab,
        Self::Octopus,
        Self::Player,
        Self::Saucer,
        Self::Explosion,
    ];

    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Self::Squid => MAGENTA,
            Self::Crab => CYAN,
            Self::Octopus => GREEN,
            Self::Player => WHITE,
            Self::Saucer => RED,
            Self::Explosion => YELLOW,
        }
    }
}

impl SpriteId {
    pub fn bounds(self, size: SpriteSize) -> (u16, u16) {
        if self == Self::Explosion {
            match size {
                SpriteSize::Small => (7, 3),
                SpriteSize::Medium => (9, 5),
                SpriteSize::Large => (13, 7),
            }
        } else if self == Self::Saucer {
            match size {
                SpriteSize::Small => (7, 2),
                SpriteSize::Medium => (10, 3),
                SpriteSize::Large => (16, 4),
            }
        } else {
            size.alien_bounds()
        }
    }

    pub fn frame_count(self) -> usize {
        if self == Self::Explosion {
            6
        } else {
            2
        }
    }
}

fn gradient(id: SpriteId, frame: usize) -> Fill {
    let color = |(r, g, b)| Color::rgb(r, g, b);
    // Retain the richer palette, with just 15% less contrast at its extremes.
    let soften = |base: (u8, u8, u8), edge: (u8, u8, u8)| {
        let mix =
            |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * 0.85).round() as u8;
        Color::rgb(
            mix(base.0, edge.0),
            mix(base.1, edge.1),
            mix(base.2, edge.2),
        )
    };
    if id == SpriteId::Explosion {
        let (core, edge) = [
            (WHITE, (255, 244, 195)),
            ((255, 252, 220), YELLOW),
            (YELLOW, (255, 137, 65)),
            ((255, 167, 80), RED),
            (RED, (158, 61, 86)),
            ((128, 72, 82), (48, 33, 54)),
        ][frame];
        return Fill::Radial(RadialGradient::new(
            (0.5, 0.5),
            (0.5, 0.5),
            0.7,
            vec![
                ColorStop::new(0.0, color(core)),
                ColorStop::new(1.0, soften(core, edge)),
            ],
        ));
    }
    let (light, shadow) = match id {
        SpriteId::Squid => ((255, 210, 249), (114, 51, 190)),
        SpriteId::Crab => ((201, 255, 255), (35, 99, 181)),
        SpriteId::Octopus => ((220, 255, 180), (29, 119, 108)),
        SpriteId::Player => ((255, 255, 255), (91, 152, 221)),
        SpriteId::Saucer => ((255, 222, 173), (160, 40, 116)),
        SpriteId::Explosion => unreachable!(),
    };
    Fill::Linear(LinearGradient::new(
        65.0,
        vec![
            ColorStop::new(0.0, soften(id.color(), light)),
            ColorStop::new(0.5, color(id.color())),
            ColorStop::new(1.0, soften(id.color(), shadow)),
        ],
    ))
}

fn explosion_art(size: SpriteSize, frame: usize) -> &'static str {
    match size {
        SpriteSize::Small => [
            "       \n   ◆   \n       ",
            "   ╵   \n  ▟█▙  \n   ╷   ",
            " ╲ │ ╱ \n ─ ◆ ─ \n ╱ │ ╲ ",
            "╲  ·  ╱\n·     ·\n╱  ·  ╲",
            "·    ˙ \n  ·    \n ˙   · ",
            "       \n ·   ˙ \n       ",
        ][frame],
        SpriteSize::Medium => [
            "         \n         \n    ◆    \n         \n         ",
            "         \n    ╵    \n   ▟█▙   \n    ╷    \n         ",
            "  ╲ │ ╱  \n   ╲│╱   \n ── ◆ ── \n   ╱│╲   \n  ╱ │ ╲  ",
            "╲   ·   ╱\n  ·   ·  \n·       ·\n  ·   ·  \n╱   ·   ╲",
            "·      ˙ \n   ·     \n ˙     · \n    ˙    \n  ·    ˙ ",
            "         \n ·     ˙ \n         \n   ˙     \n         ",
        ][frame],
        SpriteSize::Large => [
            "             \n             \n             \n      ◆      \n             \n             \n             ",
            "             \n             \n      ╵      \n     ▟█▙     \n      ╷      \n             \n             ",
            "   ╲  │  ╱   \n    ╲ │ ╱    \n     ╲│╱     \n ──── ◆ ──── \n     ╱│╲     \n    ╱ │ ╲    \n   ╱  │  ╲   ",
            "╲     ·     ╱\n  ·       ·  \n    ·   ·    \n·           ·\n    ·   ·    \n  ·       ·  \n╱     ·     ╲",
            "·         ˙  \n    ·        \n  ˙      ·   \n      ˙      \n ·        ˙  \n    ˙   ·    \n  ·        ˙ ",
            "             \n  ·       ˙  \n             \n     ˙       \n             \n        ·    \n             ",
        ][frame],
    }
}

// Two marching frames per size; explosions use their own longer sequence.
fn art(id: SpriteId, size: SpriteSize) -> [&'static str; 2] {
    use SpriteId::*;
    use SpriteSize::*;
    match (id, size) {
        (Squid, Small) => [" ▄█▄ \n▟▀ ▀▙", " ▄█▄ \n▘▀ ▀▝"],
        (Crab, Small) => ["▚▄ ▄▞\n ▜█▛ ", "▗▄ ▄▖\n▘▜█▛▝"],
        (Octopus, Small) => ["▗███▖\n▛▙ ▟▜", "▗███▖\n▙▘ ▝▟"],
        (Player, Small) => ["  ▄  \n▄███▄"; 2],
        (Saucer, Small) => [" ▄███▄ \n▀▟▀▟▀▟▀", " ▄███▄ \n▀▙▀▙▀▙▀"],
        (Squid, Medium) => ["  ▄█▄  \n ▟███▙ \n▟▀   ▀▙", "  ▄█▄  \n ▟███▙ \n ▀▙ ▟▀ "],
        (Crab, Medium) => [" ▚   ▞ \n▗▄█▄█▄▖\n▘ ▜█▛ ▝", " ▗   ▖ \n▟▄█▄█▄▙\n ▘▜█▛▝ "],
        (Octopus, Medium) => [" ▄███▄ \n▟█▀█▀█▙\n▛▙   ▟▜", " ▄███▄ \n▟█▀█▀█▙\n▙▘▝ ▘▝▟"],
        (Player, Medium) => ["   ▄   \n ▄███▄ \n▟█████▙"; 2],
        (Saucer, Medium) => [
            "   ▄██▄   \n ▄██████▄ \n▀▟▀▟▀▟▀▟▀▀",
            "   ▄██▄   \n ▄██████▄ \n▀▙▀▙▀▙▀▙▀▀",
        ],
        (Squid, Large) => [
            "    ▄█▄    \n  ▄█████▄  \n ▟█▀█▀█▀█▙ \n▟▀ ▀   ▀ ▀▙",
            "    ▄█▄    \n  ▄█████▄  \n ▟█▀█▀█▀█▙ \n ▀▙     ▟▀ ",
        ],
        (Crab, Large) => [
            " ▚       ▞ \n  ▚▄███▄▞  \n▟██▀███▀██▙\n▘ ▜▀   ▀▛ ▝",
            " ▗       ▖ \n ▟▘▄███▄▝▙ \n▟██▀███▀██▙\n  ▘▜   ▛▝  ",
        ],
        (Octopus, Large) => [
            "  ▄█████▄  \n ▟███████▙ \n██▀▀███▀▀██\n▛▙ ▟▀ ▀▙ ▟▜",
            "  ▄█████▄  \n ▟███████▙ \n██▀▀███▀▀██\n▙▘ ▝▙ ▟▘ ▝▟",
        ],
        (Player, Large) => ["     ▄     \n    ▟█▙    \n ▄███████▄ \n▟█████████▙"; 2],
        (Saucer, Large) => [
            "     ▄███▄      \n   ▄███████▄    \n ▄███████████▄  \n▀▟▀▟▀▟▀▟▀▟▀▟▀▟▀▀",
            "     ▄███▄      \n   ▄███████▄    \n ▄███████████▄  \n▀▙▀▙▀▙▀▙▀▙▀▙▀▙▀▀",
        ],
        (Explosion, _) => unreachable!("explosions use the six-stage sequence"),
    }
}

struct CachedFrame {
    id: SpriteId,
    size: SpriteSize,
    frame: usize,
    rendered: SpriteRendered,
}

/// Artbox does composition and coloring once. The hot path only blits its grid.
pub struct SpriteAtlas {
    frames: Vec<CachedFrame>,
}

impl Default for SpriteAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteAtlas {
    pub fn new() -> Self {
        let mut frames = Vec::new();
        for id in SpriteId::ALL {
            for frame in 0..id.frame_count() {
                let mut variants = Vec::new();
                for size in SpriteSize::ALL.into_iter().rev() {
                    let content = if id == SpriteId::Explosion {
                        explosion_art(size, frame)
                    } else {
                        art(id, size)[frame]
                    };
                    variants.push(SpriteVariant::new(
                        size.id(),
                        vec![SpriteLayer::new(content).with_fill(gradient(id, frame))],
                    ));
                }
                let sprite = Sprite::new(variants);
                for size in SpriteSize::ALL {
                    let (width, height) = id.bounds(size);
                    let rendered = sprite
                        .render_with(width, height, SpriteSelection::Id(size.id()))
                        .expect("embedded sprite must fit atlas bounds");
                    frames.push(CachedFrame {
                        id,
                        size,
                        frame,
                        rendered,
                    });
                }
            }
        }
        Self { frames }
    }

    pub fn get(&self, id: SpriteId, size: SpriteSize, frame: usize) -> &SpriteRendered {
        &self
            .frames
            .iter()
            .find(|entry| {
                entry.id == id && entry.size == size && entry.frame == frame % id.frame_count()
            })
            .expect("all sprite variants are cached")
            .rendered
    }
}
