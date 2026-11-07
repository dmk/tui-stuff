use ratatui::layout::Rect;

use crate::sprites::{SpriteSize, SpriteSizing};

#[derive(Clone, Copy, Debug)]
pub struct BoardLayout {
    pub size: SpriteSize,
    pub board: Rect,
}

impl BoardLayout {
    pub fn fit(area: Rect) -> Option<Self> {
        Self::with_sizing(area, SpriteSizing::Auto)
    }

    pub fn with_sizing(area: Rect, sizing: SpriteSizing) -> Option<Self> {
        let fitted = SpriteSize::fit(area.width, area.height)?;
        let size = match sizing {
            SpriteSizing::Compact => SpriteSize::Small,
            SpriteSizing::Auto => fitted,
        };
        let (width, height) = size.board_bounds();
        Some(Self {
            size,
            board: Rect::new(
                area.x + (area.width - width) / 2,
                area.y + (area.height - height - 2) / 2 + 1,
                width,
                height,
            ),
        })
    }

    pub fn point(self, x: i32, y: i32) -> (i32, i32) {
        let (w, h) = self.size.alien_bounds();
        (
            i32::from(self.board.x) + (x * i32::from(w)).div_euclid(5),
            i32::from(self.board.y) + (y * i32::from(h)).div_euclid(4),
        )
    }
}
