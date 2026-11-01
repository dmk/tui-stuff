use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use std::rc::Rc;
use tui_dispatch::{Component, EventKind, EventOutcome};
use tui_dispatch_components::{style::BorderStyle, Modal, ModalBehavior, ModalProps, ModalStyle};

use crate::{
    action::{Action, Direction},
    sprites::{self, SpriteAtlas, SpriteId, SpriteSize, SpriteSizing},
    state::{AlienKind, AppState, Owner, Phase, Shield, MIN_HEIGHT, MIN_WIDTH, PLAYER_Y},
};

const BACKGROUND: Color = Color::Rgb(8, 10, 17);
const MUTED: Color = Color::Rgb(125, 142, 165);

fn rgb(value: (u8, u8, u8)) -> Color {
    Color::Rgb(value.0, value.1, value.2)
}

pub fn map_event(event: &EventKind, state: &AppState) -> EventOutcome<Action> {
    if let EventKind::Resize(width, height) = event {
        return EventOutcome::action(Action::ViewportResize {
            width: *width,
            height: *height,
        })
        .with_render();
    }
    let mut component = Controls;
    EventOutcome::from_actions(component.handle_event(event, state))
}

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

fn put(frame: &mut Frame, clip: Rect, x: i32, y: i32, ch: char, color: Color) {
    if x >= i32::from(clip.x)
        && y >= i32::from(clip.y)
        && x < i32::from(clip.right())
        && y < i32::from(clip.bottom())
    {
        if let Some(cell) = frame.buffer_mut().cell_mut((x as u16, y as u16)) {
            cell.set_char(ch).set_fg(color).set_bg(BACKGROUND);
        }
    }
}

pub struct Controls;

impl Component<Action> for Controls {
    type Props<'a> = &'a AppState;

    fn handle_event(
        &mut self,
        event: &EventKind,
        state: Self::Props<'_>,
    ) -> impl IntoIterator<Item = Action> {
        let EventKind::Key(key) = event else {
            return None;
        };
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(Action::Quit);
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('a' | 'A') => Some(Action::PlayerMove(Direction::Left)),
            KeyCode::Right | KeyCode::Char('d' | 'D') => Some(Action::PlayerMove(Direction::Right)),
            KeyCode::Char(' ') => Some(Action::PlayerFire),
            KeyCode::Enter if state.phase == Phase::Title => Some(Action::GameStart),
            KeyCode::Enter if state.phase == Phase::GameOver => Some(Action::GameRestart),
            KeyCode::Char('p' | 'P') | KeyCode::Esc => Some(Action::PauseToggle),
            KeyCode::Char('r' | 'R') => Some(Action::GameRestart),
            KeyCode::Char('q' | 'Q') => Some(Action::Quit),
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, _props: Self::Props<'_>) {
        frame.render_widget(
            Paragraph::new("←/→ A/D move  SPACE fire  P pause  R restart  Q quit  F12 debug")
                .centered()
                .style(Style::default().fg(MUTED)),
            area,
        );
    }
}

pub struct Playfield;
pub struct PlayfieldProps<'a> {
    pub state: &'a AppState,
    pub atlas: &'a SpriteAtlas,
    pub layout: BoardLayout,
}

impl Component<Action> for Playfield {
    type Props<'a> = PlayfieldProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: PlayfieldProps<'_>) {
        let PlayfieldProps {
            state,
            atlas,
            layout,
        } = props;
        // Sparse deterministic stars; background has no simulation state.
        for (x, y) in [(2, 3), (19, 1), (61, 11), (37, 27), (15, 25), (57, 29)] {
            let (x, y) = layout.point(x, y);
            put(frame, area, x, y, '·', Color::Rgb(40, 49, 68));
        }
        let mut draw = |id, x, y, animation, tint| {
            let (x, y) = layout.point(x, y);
            SpriteComponent.render(
                frame,
                area,
                SpriteProps {
                    atlas,
                    id,
                    size: layout.size,
                    frame: animation,
                    x,
                    y,
                    tint,
                },
            );
        };
        for alien in &state.aliens {
            let id = match alien.kind {
                AlienKind::Squid => SpriteId::Squid,
                AlienKind::Crab => SpriteId::Crab,
                AlienKind::Octopus => SpriteId::Octopus,
            };
            draw(id, alien.x, alien.y, state.formation_frame, None);
        }
        if let Some(saucer) = &state.saucer {
            draw(
                SpriteId::Saucer,
                saucer.x,
                saucer.y,
                (state.tick / 8) as usize,
                None,
            );
        }
        let player_visible = !matches!(state.phase, Phase::Respawning | Phase::GameOver)
            && (state.player.invulnerable == 0 || (state.tick / 5).is_multiple_of(2));
        if player_visible {
            draw(
                SpriteId::Player,
                state.player.x,
                PLAYER_Y,
                0,
                (state.player.invulnerable > 0).then_some(rgb(sprites::CYAN)),
            );
        }
        for shield in &state.shields {
            render_shield(frame, area, layout, shield);
        }
        for bullet in &state.bullets {
            let (x, y) = layout.point(bullet.x, bullet.y);
            let (glyph, color) = match bullet.owner {
                Owner::Player => ('╵', sprites::YELLOW),
                Owner::Alien => (
                    if (state.tick / 4).is_multiple_of(2) {
                        '╲'
                    } else {
                        '╱'
                    },
                    sprites::RED,
                ),
            };
            put(frame, area, x, y, glyph, rgb(color));
        }
        // Bursts expand around the original entity center, rather than shifting
        // their anchor as each animation frame grows. They draw above shields.
        for explosion in &state.explosions {
            let (x, y) = layout.point(explosion.x, explosion.y);
            let (entity_w, entity_h) = layout.size.alien_bounds();
            let (burst_w, burst_h) = SpriteId::Explosion.bounds(layout.size);
            SpriteComponent.render(
                frame,
                area,
                SpriteProps {
                    atlas,
                    id: SpriteId::Explosion,
                    size: layout.size,
                    frame: explosion.frame(),
                    x: x + (i32::from(entity_w) - i32::from(burst_w)) / 2,
                    y: y + (i32::from(entity_h) - i32::from(burst_h)) / 2,
                    tint: None,
                },
            );
        }
    }
}

fn render_shield(frame: &mut Frame, clip: Rect, layout: BoardLayout, shield: &Shield) {
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
            put(frame, clip, x + col, y + row, glyph, color);
        }
    }
}

pub struct Hud;
pub struct HudProps<'a> {
    pub state: &'a AppState,
    pub atlas: &'a SpriteAtlas,
}

impl Component<Action> for Hud {
    type Props<'a> = HudProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: HudProps<'_>) {
        let text = Line::from(vec![
            Span::styled("SCORE ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{:06}", props.state.score),
                Style::default().fg(rgb(sprites::WHITE)),
            ),
            Span::styled(
                format!("    WAVE {:02}", props.state.wave),
                Style::default().fg(rgb(sprites::CYAN)),
            ),
        ]);
        frame.render_widget(Paragraph::new(text), area);
        let x = i32::from(area.right()) - 6 * i32::from(props.state.lives) - 6;
        for life in 0..props.state.lives {
            // Crop the barrel row: the five-cell hull makes a compact life icon.
            SpriteComponent.render(
                frame,
                area,
                SpriteProps {
                    atlas: props.atlas,
                    id: SpriteId::Player,
                    size: SpriteSize::Small,
                    frame: 0,
                    x: x + i32::from(life) * 6,
                    y: i32::from(area.y) - 1,
                    tint: Some(rgb(sprites::GREEN)),
                },
            );
        }
        frame.render_widget(
            Paragraph::new("LIVES").style(Style::default().fg(MUTED)),
            Rect::new(area.right().saturating_sub(5), area.y, 5, 1),
        );
    }
}

pub struct PhaseOverlay;
pub struct OverlayProps<'a> {
    pub state: &'a AppState,
    pub atlas: &'a SpriteAtlas,
}

impl Component<Action> for PhaseOverlay {
    type Props<'a> = OverlayProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: OverlayProps<'_>) {
        let state = props.state;
        let (title, lines, height) = match state.phase {
            Phase::Title => (
                " SPACE INVADERS ",
                vec![
                    "A small invasion. A very small ship.".to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                    "ENTER  launch       Q  quit".to_string(),
                ],
                9,
            ),
            Phase::Paused => (
                " PAUSED ",
                vec![
                    "P / ESC  resume".to_string(),
                    "R  restart       Q  quit".to_string(),
                ],
                5,
            ),
            Phase::GameOver => (
                " GAME OVER ",
                vec![
                    format!("SCORE {:06}    WAVE {}", state.score, state.wave),
                    "ENTER / R  play again".to_string(),
                ],
                5,
            ),
            Phase::WaveClear => (
                " SECTOR CLEAR ",
                vec![format!("Wave {} incoming…", state.wave + 1)],
                4,
            ),
            _ => return,
        };
        let width = 44.min(area.width);
        let height = height.min(area.height);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let mut content = |frame: &mut Frame, inner: Rect| {
            frame.render_widget(
                Paragraph::new(title).style(Style::default().fg(rgb(sprites::CYAN)).bg(BACKGROUND)),
                Rect::new(
                    popup.x + 2,
                    popup.y,
                    (title.chars().count() as u16).min(popup.width.saturating_sub(4)),
                    1,
                ),
            );
            frame.render_widget(
                Paragraph::new(lines.join("\n"))
                    .centered()
                    .style(Style::default().fg(rgb(sprites::WHITE))),
                inner,
            );
            if state.phase == Phase::Title {
                for (offset, id) in [
                    (8, SpriteId::Squid),
                    (19, SpriteId::Crab),
                    (30, SpriteId::Octopus),
                ] {
                    SpriteComponent.render(
                        frame,
                        inner,
                        SpriteProps {
                            atlas: props.atlas,
                            id,
                            size: SpriteSize::Small,
                            frame: 0,
                            x: i32::from(popup.x + offset),
                            y: i32::from(popup.y + 3),
                            tint: None,
                        },
                    );
                }
            }
        };
        let border = BorderStyle {
            style: Style::default().fg(rgb(sprites::CYAN)),
            focused_style: None,
            ..BorderStyle::default()
        };
        Modal.render(
            frame,
            area,
            ModalProps {
                is_open: true,
                is_focused: true,
                area: popup,
                style: ModalStyle {
                    dim_factor: 0.68,
                    ..ModalStyle::with_bg_and_border(BACKGROUND, border)
                },
                behavior: ModalBehavior {
                    close_on_esc: state.phase == Phase::Paused,
                    close_on_backdrop: false,
                },
                on_close: Rc::new(|| Action::PauseToggle),
                render_content: &mut content,
            },
        );
    }
}

#[derive(Default)]
pub struct GameUi {
    pub atlas: SpriteAtlas,
    pub sizing: SpriteSizing,
}

impl GameUi {
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        frame.render_widget(
            Block::default().style(Style::default().bg(BACKGROUND)),
            area,
        );
        let Some(layout) = BoardLayout::with_sizing(area, self.sizing) else {
            frame.render_widget(
                Paragraph::new(format!("SPACE INVADERS\nResize to at least {MIN_WIDTH} × {MIN_HEIGHT}\nSimulation suspended · Q to quit"))
                .centered()
                .style(Style::default().fg(rgb(sprites::CYAN))),
                Rect::new(
                    area.x,
                    area.y + area.height.saturating_sub(3) / 2,
                    area.width,
                    area.height.min(3),
                ),
            );
            return;
        };
        Hud.render(
            frame,
            Rect::new(layout.board.x, layout.board.y - 1, layout.board.width, 1),
            HudProps {
                state,
                atlas: &self.atlas,
            },
        );
        Playfield.render(
            frame,
            layout.board,
            PlayfieldProps {
                state,
                atlas: &self.atlas,
                layout,
            },
        );
        Controls.render(
            frame,
            Rect::new(area.x, layout.board.bottom(), area.width, 1),
            state,
        );
        // Render the modal last so it dims the playfield, HUD, and controls.
        PhaseOverlay.render(
            frame,
            layout.board,
            OverlayProps {
                state,
                atlas: &self.atlas,
            },
        );
    }
}
