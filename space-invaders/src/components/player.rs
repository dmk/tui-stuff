use ratatui::{layout::Rect, style::Color, Frame};
use tui_dispatch::Component;

use super::{BoardLayout, SpriteComponent, SpriteProps};
use crate::{
    action::Action,
    sprites::{self, SpriteAtlas, SpriteId},
    state::{Phase, Player, PLAYER_Y, POSITION_SCALE},
};

pub struct PlayerComponent;

pub struct PlayerProps<'a> {
    pub player: &'a Player,
    pub atlas: &'a SpriteAtlas,
    pub layout: BoardLayout,
    pub phase: Phase,
    pub tick: u64,
}

impl Component<Action> for PlayerComponent {
    type Props<'a> = PlayerProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: PlayerProps<'_>) {
        let player = props.player;
        let visible = !matches!(props.phase, Phase::Respawning | Phase::GameOver)
            && (player.invulnerable == 0 || (props.tick / 5).is_multiple_of(2));
        if !visible {
            return;
        }
        let (w, _) = props.layout.size.alien_bounds();
        let x = i32::from(props.layout.board.x)
            + (player.x * POSITION_SCALE + player.motion_fraction) * i32::from(w)
                / (5 * POSITION_SCALE);
        let (_, y) = props.layout.point(player.x, PLAYER_Y);
        let (r, g, b) = sprites::CYAN;
        SpriteComponent.render(
            frame,
            area,
            SpriteProps {
                atlas: props.atlas,
                id: SpriteId::Player,
                size: props.layout.size,
                frame: 0,
                x,
                y,
                tint: (player.invulnerable > 0).then_some(Color::Rgb(r, g, b)),
            },
        );
    }
}
