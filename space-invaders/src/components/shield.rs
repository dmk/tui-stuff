use ratatui::{layout::Rect, style::Color, Frame};
use tui_dispatch::Component;

use super::{sprite::put, BoardLayout};
use crate::{action::Action, state::Shield};

pub struct ShieldComponent;

pub struct ShieldProps<'a> {
    pub shield: &'a Shield,
    pub layout: BoardLayout,
}

impl Component<Action> for ShieldComponent {
    type Props<'a> = ShieldProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: ShieldProps<'_>) {
        let ShieldProps { shield, layout } = props;
        let (x, y) = layout.point(shield.x, shield.y);
        let (w, h) = layout.size.alien_bounds();
        let width = (7 * i32::from(w) + 4) / 5;
        let height = (6 * i32::from(h) + 3) / 4;
        for row in 0..height {
            for col in 0..width {
                let source_x = shield.x + col * 7 / width;
                let top = shield.occupied(source_x, shield.y + row * 2 * 6 / (height * 2));
                let bottom = shield.occupied(source_x, shield.y + (row * 2 + 1) * 6 / (height * 2));
                let glyph = match (top, bottom) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    _ => continue,
                };
                let t = row as f32 / (height - 1).max(1) as f32;
                let channel = |top: f32, bottom: f32| (top + (bottom - top) * t) as u8;
                let color = Color::Rgb(
                    channel(187.0, 41.0),
                    channel(254.0, 138.0),
                    channel(171.0, 114.0),
                );
                put(frame, area, x + col, y + row, glyph, color);
            }
        }
    }
}
