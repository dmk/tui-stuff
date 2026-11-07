use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use std::rc::Rc;
use tui_dispatch::{Component, EventKind, EventOutcome};
use tui_dispatch_components::{Modal, ModalBehavior, ModalProps, ModalStyle};

use crate::{
    action::{Action, Direction, MovementKey},
    components::{
        sprite::{bounds, put},
        AlienComponent, AlienProps, BulletComponent, BulletProps, ComponentRegistry,
        ExplosionComponent, ExplosionProps, PlayerComponent, PlayerProps, SaucerComponent,
        SaucerProps, ShieldComponent, ShieldProps,
    },
    input::KeyboardMode,
    sprites::{self, SpriteAtlas, SpriteId, SpriteSize, SpriteSizing},
    state::{AppState, Phase, MIN_HEIGHT, MIN_WIDTH, PLAYER_Y, POSITION_SCALE},
};

pub use crate::components::{BoardLayout, SpriteComponent, SpriteProps};

// Use the terminal's own background, including light themes.
const BACKGROUND: Color = Color::Reset;
const MUTED: Color = Color::Rgb(125, 142, 165);

pub fn map_event(event: &EventKind, state: &AppState) -> EventOutcome<Action> {
    map_event_with_mode(event, state, KeyboardMode::Legacy)
}

pub fn map_event_with_mode(
    event: &EventKind,
    state: &AppState,
    mode: KeyboardMode,
) -> EventOutcome<Action> {
    if let EventKind::Resize(width, height) = event {
        return EventOutcome::action(Action::ViewportResize {
            width: *width,
            height: *height,
        })
        .with_render();
    }
    let mut component = Controls { mode };
    EventOutcome::from_actions(component.handle_event(event, state))
}

fn rgb(value: (u8, u8, u8)) -> Color {
    Color::Rgb(value.0, value.1, value.2)
}

#[derive(Default)]
pub struct Controls {
    pub mode: KeyboardMode,
}

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
        if self.mode == KeyboardMode::Held {
            let movement = match key.code {
                KeyCode::Left => Some(MovementKey::Left),
                KeyCode::Char('a' | 'A') => Some(MovementKey::A),
                KeyCode::Right => Some(MovementKey::Right),
                KeyCode::Char('d' | 'D') => Some(MovementKey::D),
                _ => None,
            };
            if let Some(movement) = movement {
                return match key.kind {
                    KeyEventKind::Press => Some(Action::PlayerKeyDown(movement)),
                    KeyEventKind::Release => Some(Action::PlayerKeyUp(movement)),
                    KeyEventKind::Repeat => None,
                };
            }
        }
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if key.kind == KeyEventKind::Repeat
            && key.code != KeyCode::Char(' ')
            && self.mode == KeyboardMode::Held
        {
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
    pub components: &'a ComponentRegistry,
}

impl Component<Action> for Playfield {
    type Props<'a> = PlayfieldProps<'a>;

    fn render(&mut self, frame: &mut Frame, area: Rect, props: PlayfieldProps<'_>) {
        let PlayfieldProps {
            state,
            atlas,
            layout,
            components,
        } = props;
        // Sparse deterministic stars; background has no simulation state.
        for (x, y) in [(2, 3), (19, 1), (61, 11), (37, 27), (15, 25), (57, 29)] {
            let (x, y) = layout.point(x, y);
            put(frame, area, x, y, '·', Color::Rgb(40, 49, 68));
        }
        for (index, alien) in state.aliens.iter().enumerate() {
            components.render(
                AlienComponent,
                frame,
                bounds(
                    area,
                    layout.point(alien.x, alien.y),
                    layout.size.alien_bounds(),
                ),
                AlienProps {
                    alien,
                    atlas,
                    layout,
                    animation: state.formation_frame,
                },
                format!("alien[{index}]"),
                alien,
            );
        }
        if let Some(saucer) = &state.saucer {
            components.render(
                SaucerComponent,
                frame,
                bounds(
                    area,
                    layout.point(saucer.x, saucer.y),
                    SpriteId::Saucer.bounds(layout.size),
                ),
                SaucerProps {
                    saucer,
                    atlas,
                    layout,
                    tick: state.tick,
                },
                "saucer",
                saucer,
            );
        }
        let (entity_width, entity_height) = layout.size.alien_bounds();
        let player_x = i32::from(layout.board.x)
            + (state.player.x * POSITION_SCALE + state.player.motion_fraction)
                * i32::from(entity_width)
                / (5 * POSITION_SCALE);
        let (_, player_y) = layout.point(state.player.x, PLAYER_Y);
        components.render(
            PlayerComponent,
            frame,
            bounds(area, (player_x, player_y), (entity_width, entity_height)),
            PlayerProps {
                player: &state.player,
                atlas,
                layout,
                phase: state.phase,
                tick: state.tick,
            },
            "player",
            &state.player,
        );
        for (index, shield) in state.shields.iter().enumerate() {
            components.render(
                ShieldComponent,
                frame,
                bounds(
                    area,
                    layout.point(shield.x, shield.y),
                    (
                        (7 * u32::from(entity_width)).div_ceil(5) as u16,
                        (6 * u32::from(entity_height)).div_ceil(4) as u16,
                    ),
                ),
                ShieldProps { shield, layout },
                format!("shield[{index}]"),
                shield,
            );
        }
        for (index, bullet) in state.bullets.iter().enumerate() {
            components.render(
                BulletComponent,
                frame,
                bounds(area, layout.point(bullet.x, bullet.y), (1, 1)),
                BulletProps {
                    bullet,
                    layout,
                    tick: state.tick,
                },
                format!("bullet[{index}]"),
                bullet,
            );
        }
        // Explosions draw last so their bursts appear above shields and shots.
        for (index, explosion) in state.explosions.iter().enumerate() {
            let (x, y) = layout.point(explosion.x, explosion.y);
            let (width, height) = SpriteId::Explosion.bounds(layout.size);
            let position = (
                x + (i32::from(entity_width) - i32::from(width)) / 2,
                y + (i32::from(entity_height) - i32::from(height)) / 2,
            );
            components.render(
                ExplosionComponent,
                frame,
                bounds(area, position, (width, height)),
                ExplosionProps {
                    explosion,
                    atlas,
                    layout,
                },
                format!("explosion[{index}]"),
                explosion,
            );
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
                Style::default().fg(Color::Reset),
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
                "SPACE INVADERS",
                vec![
                    "A small invasion. A very small ship.".to_string(),
                    "ENTER launch · Q quit".to_string(),
                ],
                10,
            ),
            Phase::Paused => (
                "PAUSED",
                vec![
                    "P / ESC  resume".to_string(),
                    String::new(),
                    "R  restart       Q  quit".to_string(),
                ],
                7,
            ),
            Phase::GameOver => (
                "GAME OVER",
                vec![
                    format!("SCORE {:06}    WAVE {}", state.score, state.wave),
                    String::new(),
                    "ENTER / R  play again".to_string(),
                ],
                7,
            ),
            Phase::WaveClear => (
                "SECTOR CLEAR",
                vec![format!("Wave {} incoming…", state.wave + 1)],
                5,
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
            let [heading, _, body] = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .areas(inner.inner(Margin::new(2, 1)));
            frame.render_widget(
                Paragraph::new(title).centered().style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                heading,
            );
            if state.phase == Phase::Title {
                let [tagline, _, previews, _, controls] = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .areas(body);
                frame.render_widget(
                    Paragraph::new(lines[0].as_str())
                        .centered()
                        .style(Style::default().fg(Color::Gray)),
                    tagline,
                );
                frame.render_widget(
                    Paragraph::new(lines[1].as_str())
                        .centered()
                        .style(Style::default().fg(Color::White)),
                    controls,
                );
                let columns: [Rect; 3] =
                    Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(previews);
                for (column, id) in
                    columns
                        .into_iter()
                        .zip([SpriteId::Squid, SpriteId::Crab, SpriteId::Octopus])
                {
                    let (width, _) = id.bounds(SpriteSize::Small);
                    SpriteComponent.render(
                        frame,
                        column,
                        SpriteProps {
                            atlas: props.atlas,
                            id,
                            size: SpriteSize::Small,
                            frame: 0,
                            x: i32::from(column.x + column.width.saturating_sub(width) / 2),
                            y: i32::from(column.y),
                            tint: None,
                        },
                    );
                }
            } else {
                frame.render_widget(
                    Paragraph::new(lines.join("\n"))
                        .centered()
                        .style(Style::default().fg(Color::White)),
                    body,
                );
            }
        };
        Modal.render(
            frame,
            area,
            ModalProps {
                is_open: true,
                is_focused: true,
                area: popup,
                style: ModalStyle::with_bg(Color::Rgb(35, 35, 45)),
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
    pub components: ComponentRegistry,
}

impl GameUi {
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        self.components.begin_frame();
        self.components.record::<Self>(
            area,
            "game",
            &serde_json::json!({
                "phase": state.phase,
                "viewport_width": state.viewport_width,
                "viewport_height": state.viewport_height,
                "sizing": format!("{:?}", self.sizing),
            }),
        );
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
        self.components.render(
            Hud,
            frame,
            Rect::new(layout.board.x, layout.board.y - 1, layout.board.width, 1),
            HudProps {
                state,
                atlas: &self.atlas,
            },
            "hud",
            &serde_json::json!({"score": state.score, "lives": state.lives, "wave": state.wave}),
        );
        self.components.render(
            Playfield,
            frame,
            layout.board,
            PlayfieldProps {
                state,
                atlas: &self.atlas,
                layout,
                components: &self.components,
            },
            "playfield",
            &serde_json::json!({
                "tick": state.tick,
                "aliens": state.aliens.len(),
                "bullets": state.bullets.len(),
                "explosions": state.explosions.len(),
                "formation_frame": state.formation_frame,
                "formation_direction": state.formation_direction,
            }),
        );
        self.components.render(
            Controls::default(),
            frame,
            Rect::new(area.x, layout.board.bottom(), area.width, 1),
            state,
            "controls",
            &serde_json::json!({"held_keys": state.player.held_keys}),
        );
        // Render the modal last so it dims the playfield, HUD, and controls.
        self.components.render(
            PhaseOverlay,
            frame,
            layout.board,
            OverlayProps {
                state,
                atlas: &self.atlas,
            },
            "phase-overlay",
            &serde_json::json!({"phase": state.phase, "phase_timer": state.phase_timer}),
        );
    }
}
