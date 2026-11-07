use ratatui::{layout::Rect, Frame};
use tui_dispatch::Component;

use super::{BoardLayout, SpriteComponent, SpriteProps};
use crate::{
    action::Action,
    sprites::{SpriteAtlas, SpriteId},
    state::{Alien, AlienKind},
};

pub struct AlienComponent;

pub struct AlienProps<'a> {
    pub alien: &'a Alien,
    pub atlas: &'a SpriteAtlas,
    pub layout: BoardLayout,
    pub animation: usize,
}

impl Component<Action> for AlienComponent {
    type Props<'a> = AlienProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: AlienProps<'_>) {
        let id = match props.alien.kind {
            AlienKind::Squid => SpriteId::Squid,
            AlienKind::Crab => SpriteId::Crab,
            AlienKind::Octopus => SpriteId::Octopus,
        };
        let (x, y) = props.layout.point(props.alien.x, props.alien.y);
        SpriteComponent.render(
            frame,
            area,
            SpriteProps {
                atlas: props.atlas,
                id,
                size: props.layout.size,
                frame: props.animation,
                x,
                y,
                tint: None,
            },
        );
    }
}
