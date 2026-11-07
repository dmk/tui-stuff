use ratatui::{layout::Rect, Frame};
use tui_dispatch::Component;

use super::{BoardLayout, SpriteComponent, SpriteProps};
use crate::{
    action::Action,
    sprites::{SpriteAtlas, SpriteId},
    state::Saucer,
};

pub struct SaucerComponent;

pub struct SaucerProps<'a> {
    pub saucer: &'a Saucer,
    pub atlas: &'a SpriteAtlas,
    pub layout: BoardLayout,
    pub tick: u64,
}

impl Component<Action> for SaucerComponent {
    type Props<'a> = SaucerProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: SaucerProps<'_>) {
        let (x, y) = props.layout.point(props.saucer.x, props.saucer.y);
        SpriteComponent.render(
            frame,
            area,
            SpriteProps {
                atlas: props.atlas,
                id: SpriteId::Saucer,
                size: props.layout.size,
                frame: (props.tick / 8) as usize,
                x,
                y,
                tint: None,
            },
        );
    }
}
