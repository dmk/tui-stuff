use ratatui::{layout::Rect, style::Color, Frame};
use tui_dispatch::Component;

use crate::{
    action::Action,
    sprites::{SpriteAtlas, SpriteId, SpriteSize},
};

pub struct SpriteProps<'a> {
    pub atlas: &'a SpriteAtlas,
    pub id: SpriteId,
    pub size: SpriteSize,
    pub frame: usize,
    pub x: i32,
    pub y: i32,
    pub tint: Option<Color>,
}

/// Used by the playfield, HUD life icons, and title previews.
pub struct SpriteComponent;

impl Component<Action> for SpriteComponent {
    type Props<'a> = SpriteProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: SpriteProps<'_>) {
        let sprite = props.atlas.get(props.id, props.size, props.frame);
        for (row, cells) in sprite.chars.iter().enumerate() {
            for (column, value) in cells.iter().enumerate() {
                if value.ch == ' ' {
                    continue;
                }
                let color = props
                    .tint
                    .or_else(|| value.fg.map(|c| Color::Rgb(c.r, c.g, c.b)));
                put(
                    frame,
                    area,
                    props.x + column as i32,
                    props.y + row as i32,
                    value.ch,
                    color.unwrap_or(Color::White),
                );
            }
        }
    }
}

/// Visible screen bounds for an entity positioned in signed world coordinates.
/// Clip before converting to `u16`, since saucers and explosions can enter from
/// outside the board.
pub(crate) fn bounds(clip: Rect, (x, y): (i32, i32), (width, height): (u16, u16)) -> Rect {
    let left = x.clamp(i32::from(clip.x), i32::from(clip.right()));
    let top = y.clamp(i32::from(clip.y), i32::from(clip.bottom()));
    let right = x
        .saturating_add(i32::from(width))
        .clamp(left, i32::from(clip.right()));
    let bottom = y
        .saturating_add(i32::from(height))
        .clamp(top, i32::from(clip.bottom()));
    Rect::new(
        left as u16,
        top as u16,
        (right - left) as u16,
        (bottom - top) as u16,
    )
}

pub(crate) fn put(frame: &mut Frame, clip: Rect, x: i32, y: i32, ch: char, color: Color) {
    if x >= i32::from(clip.x)
        && y >= i32::from(clip.y)
        && x < i32::from(clip.right())
        && y < i32::from(clip.bottom())
    {
        if let Some(cell) = frame.buffer_mut().cell_mut((x as u16, y as u16)) {
            cell.set_char(ch).set_fg(color);
        }
    }
}
