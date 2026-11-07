use ratatui::{layout::Rect, Frame};
use tui_dispatch::Component;

use super::{BoardLayout, SpriteComponent, SpriteProps};
use crate::{
    action::Action,
    sprites::{SpriteAtlas, SpriteId},
    state::Explosion,
};

pub struct ExplosionComponent;

pub struct ExplosionProps<'a> {
    pub explosion: &'a Explosion,
    pub atlas: &'a SpriteAtlas,
    pub layout: BoardLayout,
}

impl Component<Action> for ExplosionComponent {
    type Props<'a> = ExplosionProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: ExplosionProps<'_>) {
        let (x, y) = props.layout.point(props.explosion.x, props.explosion.y);
        let (entity_w, entity_h) = props.layout.size.alien_bounds();
        let (burst_w, burst_h) = SpriteId::Explosion.bounds(props.layout.size);
        // Keep expanding frames centered on the original entity.
        SpriteComponent.render(
            frame,
            area,
            SpriteProps {
                atlas: props.atlas,
                id: SpriteId::Explosion,
                size: props.layout.size,
                frame: props.explosion.frame(),
                x: x + (i32::from(entity_w) - i32::from(burst_w)) / 2,
                y: y + (i32::from(entity_h) - i32::from(burst_h)) / 2,
                tint: None,
            },
        );
    }
}
