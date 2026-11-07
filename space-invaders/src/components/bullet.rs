use ratatui::{layout::Rect, style::Color, Frame};
use tui_dispatch::Component;

use super::{sprite::put, BoardLayout};
use crate::{
    action::Action,
    sprites,
    state::{Bullet, Owner},
};

pub struct BulletComponent;

pub struct BulletProps<'a> {
    pub bullet: &'a Bullet,
    pub layout: BoardLayout,
    pub tick: u64,
}

impl Component<Action> for BulletComponent {
    type Props<'a> = BulletProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: BulletProps<'_>) {
        let (x, y) = props.layout.point(props.bullet.x, props.bullet.y);
        let (glyph, (r, g, b)) = match props.bullet.owner {
            Owner::Player => ('╵', sprites::YELLOW),
            Owner::Alien => (
                if (props.tick / 4).is_multiple_of(2) {
                    '╲'
                } else {
                    '╱'
                },
                sprites::RED,
            ),
        };
        put(frame, area, x, y, glyph, Color::Rgb(r, g, b));
    }
}
