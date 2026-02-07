use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{block::Title, Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use std::sync::OnceLock;
use tui_map::core::TileKind;
use tui_map::render::{Camera, MapRenderer, RenderConfig, TextureVariant, TilePalette, TileTheme};
use tui_dispatch::{EventKind, EventOutcome, RenderContext};

use crate::action::Action;
use crate::sprite;
use crate::sprite_backend;
use crate::state::{
    calc_hp, AppState, BattleKind, BattleStage, Direction as MoveDir, GameMode,
};

const BG_BASE: Color = Color::Rgb(24, 36, 26);
const BG_PANEL: Color = Color::Rgb(34, 58, 38);
const BG_PANEL_ALT: Color = Color::Rgb(28, 48, 32);
const BG_HEADER: Color = Color::Rgb(26, 46, 34);
const TEXT_MAIN: Color = Color::Rgb(228, 236, 214);
const TEXT_DIM: Color = Color::Rgb(172, 186, 160);
const ACCENT_GREEN: Color = Color::Rgb(104, 204, 120);
const ACCENT_GOLD: Color = Color::Rgb(222, 196, 120);
const HIGHLIGHT_BG: Color = ACCENT_GREEN;
const HIGHLIGHT_TEXT: Color = Color::Rgb(16, 26, 18);
const BORDER_ACCENT: Color = Color::Rgb(74, 98, 82);
const CELL_ASPECT: f32 = 2.0;
const MAP_TILES_V: u16 = 9;

const SPRITE_ID_PLAYER_MAP: u32 = 2;
const SPRITE_ID_ENEMY_BATTLE: u32 = 3;
const SPRITE_ID_PARTY_BASE: u32 = 40;

// Tile colors
const TILE_GRASS: Color = Color::Rgb(34, 112, 58);
const TILE_GRASS_ALT: Color = Color::Rgb(38, 120, 64);
const TILE_PATH: Color = Color::Rgb(156, 132, 76);
const TILE_PATH_ALT: Color = Color::Rgb(150, 128, 74);
const TILE_SAND: Color = Color::Rgb(194, 178, 128);
const TILE_SAND_ALT: Color = Color::Rgb(190, 174, 124);
const TILE_WALL: Color = Color::Rgb(66, 74, 66);
const TILE_WALL_ALT: Color = Color::Rgb(60, 68, 60);
const TILE_WATER: Color = Color::Rgb(48, 86, 146);
const TILE_WATER_ALT: Color = Color::Rgb(52, 92, 150);
static MAP_RENDERER: OnceLock<MapRenderer> = OnceLock::new();

fn adjust_color(color: Color, delta: i16) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let clamp = |v: i16| v.max(0).min(255) as u8;
            Color::Rgb(
                clamp(r as i16 + delta),
                clamp(g as i16 + delta),
                clamp(b as i16 + delta),
            )
        }
        other => other,
    }
}

fn poketui_map_theme() -> TileTheme {
    let grass = TilePalette::new(
        TILE_GRASS,
        TILE_GRASS_ALT,
        [
            TextureVariant::new('.', adjust_color(TILE_GRASS, 10), 6),
            TextureVariant::new('\'', adjust_color(TILE_GRASS, 6), 7),
            TextureVariant::new('`', adjust_color(TILE_GRASS, -4), 8),
        ],
    );
    let trail = TilePalette::new(
        TILE_PATH,
        TILE_PATH_ALT,
        [
            TextureVariant::new('.', adjust_color(TILE_PATH, 6), 6),
            TextureVariant::new(':', adjust_color(TILE_PATH, 4), 7),
            TextureVariant::new('\'', adjust_color(TILE_PATH, -4), 8),
        ],
    );
    let sand = TilePalette::new(
        TILE_SAND,
        TILE_SAND_ALT,
        [
            TextureVariant::new(':', adjust_color(TILE_SAND, 8), 6),
            TextureVariant::new('.', adjust_color(TILE_SAND, 4), 7),
            TextureVariant::new(',', adjust_color(TILE_SAND, -4), 8),
        ],
    );
    let wall = TilePalette::new(
        TILE_WALL,
        TILE_WALL_ALT,
        [
            TextureVariant::new('#', adjust_color(TILE_WALL, 10), 8),
            TextureVariant::new('+', adjust_color(TILE_WALL, 6), 9),
            TextureVariant::new('.', adjust_color(TILE_WALL, 4), 10),
        ],
    );
    let water = TilePalette::new(
        TILE_WATER,
        TILE_WATER_ALT,
        [
            TextureVariant::new('~', adjust_color(TILE_WATER, 10), 8),
            TextureVariant::new('-', adjust_color(TILE_WATER, 6), 9),
            TextureVariant::new('.', adjust_color(TILE_WATER, 4), 10),
        ],
    );

    TileTheme::builder()
        .fallback(grass)
        .tile(TileKind::Grass, grass)
        .tile(TileKind::Trail, trail)
        .tile(TileKind::Floor, trail)
        .tile(TileKind::Sand, sand)
        .tile(TileKind::Wall, wall)
        .tile(TileKind::Water, water)
        .build()
}

fn map_renderer() -> &'static MapRenderer {
    MAP_RENDERER.get_or_init(|| {
        MapRenderer::builder()
            .config(RenderConfig {
                map_tiles_vertical_hint: MAP_TILES_V,
                cell_aspect: CELL_ASPECT,
            })
            .theme(poketui_map_theme())
            .build()
    })
}

const SPRITE_ID_STARTER_PREVIEW: u32 = 5;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
    sprite_backend::clear_sprites();
    frame.render_widget(Block::default().style(Style::default().bg(BG_BASE)), area);
    match state.mode {
        GameMode::MainMenu => render_main_menu(frame, area, state),
        GameMode::PokemonSelect => render_pokemon_select(frame, area, state),
        GameMode::Overworld => {
            render_overworld(frame, area, state);
            if state.pause_menu.is_open {
                render_pause_menu(frame, area, state);
            }
        }
        GameMode::Battle => {
            render_battle(frame, area, state);
            if state.pause_menu.is_open {
                render_pause_menu(frame, area, state);
            }
        }
    }
    if state.message.is_some() {
        render_message_modal(frame, area, state);
    }
}

pub fn handle_event(event: &EventKind, state: &AppState) -> EventOutcome<Action> {
    match event {
        EventKind::Resize(width, height) => {
            EventOutcome::action(Action::UiTerminalResize(*width, *height)).with_render()
        }
        EventKind::Key(key) => handle_key(*key, state),
        _ => EventOutcome::ignored(),
    }
}

fn handle_key(key: KeyEvent, state: &AppState) -> EventOutcome<Action> {
    if state.message.is_some() {
        return match key.code {
            KeyCode::Enter | KeyCode::Char('z') | KeyCode::Char('Z') | KeyCode::Char(' ') => {
                EventOutcome::action(Action::MessageNext)
            }
            _ => EventOutcome::ignored(),
        };
    }
    // Handle pause menu if open
    if state.pause_menu.is_open {
        return handle_pause_key(key, state);
    }

    match state.mode {
        GameMode::MainMenu => handle_menu_key(key, state),
        GameMode::PokemonSelect => handle_pokemon_select_key(key, state),
        GameMode::Overworld => handle_overworld_key(key, state),
        GameMode::Battle => handle_battle_key(key, state),
    }
}

fn handle_menu_key(key: KeyEvent, state: &AppState) -> EventOutcome<Action> {
    let Some(menu) = state.menu.as_ref() else {
        return EventOutcome::ignored();
    };

    match key.code {
        KeyCode::Up | KeyCode::Char('w') => {
            let new_idx = if menu.selected == 0 {
                if menu.has_save {
                    2
                } else {
                    1
                }
            } else {
                menu.selected - 1
            };
            EventOutcome::action(Action::MenuSelect(new_idx))
        }
        KeyCode::Down | KeyCode::Char('s') => {
            let max = if menu.has_save { 2 } else { 1 };
            let new_idx = if menu.selected >= max {
                0
            } else {
                menu.selected + 1
            };
            EventOutcome::action(Action::MenuSelect(new_idx))
        }
        KeyCode::Enter | KeyCode::Char('z') | KeyCode::Char('Z') => {
            // Don't allow selecting Continue if no save
            if menu.selected == 1 && !menu.has_save {
                return EventOutcome::ignored();
            }
            let quit_index = if menu.has_save { 2 } else { 1 };
            if menu.selected == quit_index {
                return EventOutcome::action(Action::Quit);
            }
            EventOutcome::action(Action::MenuConfirm)
        }
        _ => EventOutcome::ignored(),
    }
}

fn handle_pokemon_select_key(key: KeyEvent, state: &AppState) -> EventOutcome<Action> {
    let Some(select) = state.pokemon_select.as_ref() else {
        return EventOutcome::ignored();
    };

    match key.code {
        KeyCode::Up | KeyCode::Char('w') => {
            let new_idx = if select.selected == 0 {
                select.starters.len().saturating_sub(1)
            } else {
                select.selected - 1
            };
            EventOutcome::action(Action::StarterSelect(new_idx))
        }
        KeyCode::Down | KeyCode::Char('s') => {
            let new_idx = if select.selected >= select.starters.len().saturating_sub(1) {
                0
            } else {
                select.selected + 1
            };
            EventOutcome::action(Action::StarterSelect(new_idx))
        }
        KeyCode::Enter | KeyCode::Char('z') | KeyCode::Char('Z') => {
            EventOutcome::action(Action::StarterConfirm)
        }
        KeyCode::Esc => {
            // Go back to main menu
            EventOutcome::action(Action::Init)
        }
        _ => EventOutcome::ignored(),
    }
}

fn handle_overworld_key(key: KeyEvent, _state: &AppState) -> EventOutcome<Action> {
    let action = match key.code {
        KeyCode::Up | KeyCode::Char('w') => Some(Action::Move(MoveDir::Up)),
        KeyCode::Down | KeyCode::Char('s') => Some(Action::Move(MoveDir::Down)),
        KeyCode::Left | KeyCode::Char('a') => Some(Action::Move(MoveDir::Left)),
        KeyCode::Right | KeyCode::Char('d') => Some(Action::Move(MoveDir::Right)),
        KeyCode::Esc => Some(Action::PauseOpen),
        _ => None,
    };
    EventOutcome::from(action)
}

fn handle_battle_key(key: KeyEvent, state: &AppState) -> EventOutcome<Action> {
    let Some(battle) = state.battle.as_ref() else {
        return EventOutcome::ignored();
    };

    if battle.stage == BattleStage::ItemMenu {
        let action = match key.code {
            KeyCode::Esc => Some(Action::BattleItemCancel),
            KeyCode::Enter | KeyCode::Char('z') | KeyCode::Char('Z') => Some(Action::BattleConfirm),
            KeyCode::Up | KeyCode::Left => Some(Action::BattleMenuPrev),
            KeyCode::Down | KeyCode::Right => Some(Action::BattleMenuNext),
            _ => None,
        };
        return EventOutcome::from(action);
    }

    if matches!(key.code, KeyCode::Esc) {
        return EventOutcome::action(Action::PauseOpen);
    }
    if matches!(
        key.code,
        KeyCode::Enter | KeyCode::Char('z') | KeyCode::Char('Z')
    ) {
        return EventOutcome::action(Action::BattleConfirm);
    }
    if battle.stage == BattleStage::Menu {
        let action = match key.code {
            KeyCode::Up | KeyCode::Left => Some(Action::BattleMenuPrev),
            KeyCode::Down | KeyCode::Right => Some(Action::BattleMenuNext),
            _ => None,
        };
        return EventOutcome::from(action);
    }
    EventOutcome::ignored()
}

fn handle_pause_key(key: KeyEvent, state: &AppState) -> EventOutcome<Action> {
    match key.code {
        KeyCode::Esc => EventOutcome::action(Action::PauseClose),
        KeyCode::Up | KeyCode::Char('w') => {
            let new_idx = if state.pause_menu.selected == 0 {
                2
            } else {
                state.pause_menu.selected - 1
            };
            EventOutcome::action(Action::PauseSelect(new_idx))
        }
        KeyCode::Down | KeyCode::Char('s') => {
            let new_idx = if state.pause_menu.selected >= 2 {
                0
            } else {
                state.pause_menu.selected + 1
            };
            EventOutcome::action(Action::PauseSelect(new_idx))
        }
        KeyCode::Enter | KeyCode::Char('z') | KeyCode::Char('Z') => {
            EventOutcome::action(Action::PauseConfirm)
        }
        _ => EventOutcome::ignored(),
    }
}

fn render_main_menu(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block(" POKETUI ", BG_PANEL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(menu) = state.menu.as_ref() else {
        return;
    };

    // Center the menu content
    let content_height = 10;
    let content_width = 30;
    let x = inner.x + (inner.width.saturating_sub(content_width)) / 2;
    let y = inner.y + (inner.height.saturating_sub(content_height)) / 2;
    let content_area = Rect::new(
        x,
        y,
        content_width.min(inner.width),
        content_height.min(inner.height),
    );

    let mut lines = vec![
        Line::from(Span::styled(
            "POKETUI",
            Style::default()
                .fg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "A Pokemon-inspired adventure",
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
        Line::from(""),
    ];

    // Menu options
    let options = if menu.has_save {
        vec!["New Game", "Continue", "Quit"]
    } else {
        vec!["New Game", "Quit"]
    };

    for (idx, label) in options.iter().enumerate() {
        lines.push(menu_line(label, idx == menu.selected));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Arrows/WASD: Navigate  |  Z/Enter: Select",
        Style::default().fg(TEXT_DIM),
    )));

    let paragraph = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, content_area);
}

fn render_pokemon_select(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block(" CHOOSE YOUR PARTNER ", BG_PANEL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(select) = state.pokemon_select.as_ref() else {
        return;
    };

    // Split into list (left) and preview (right)
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(20)])
        .split(inner);

    // Starter list
    let list_block = Block::default()
        .borders(Borders::ALL)
        .title("Starters")
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN))
        .border_style(Style::default().fg(BORDER_ACCENT));
    let list_inner = list_block.inner(layout[0]);
    frame.render_widget(list_block, layout[0]);

    let mut list_lines = Vec::new();
    for (idx, name) in select.starters.iter().enumerate() {
        let is_selected = idx == select.selected;
        list_lines.push(menu_line(&format_name(name), is_selected));
    }
    list_lines.push(Line::from(""));
    list_lines.push(Line::from(Span::styled(
        "ESC: Back",
        Style::default().fg(TEXT_DIM),
    )));
    let list_para = Paragraph::new(Text::from(list_lines)).wrap(Wrap { trim: true });
    frame.render_widget(list_para, list_inner);

    // Preview panel
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .title("Preview")
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN))
        .border_style(Style::default().fg(BORDER_ACCENT));
    let preview_inner = preview_block.inner(layout[1]);
    frame.render_widget(preview_block, layout[1]);

    // Split preview into sprite (top) and stats (bottom)
    let preview_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(6)])
        .split(preview_inner);

    // Render sprite
    if let Some(sprite_data) = select.preview_sprite.sprite.as_ref() {
        let sprite_area = preview_layout[0];
        let (cols, rows) = sprite_fit_scaled(
            sprite_data,
            sprite_area.width,
            sprite_area.height.saturating_sub(1),
            0.7,
        );
        let sprite_frame = sprite_data.frame(select.preview_sprite.frame_index);
        if let Ok(sequence) =
            sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_STARTER_PREVIEW)
        {
            let offset_x = sprite_area.x + (sprite_area.width.saturating_sub(cols)) / 2;
            let offset_y = sprite_area.y + (sprite_area.height.saturating_sub(rows)) / 2;
            sprite_backend::set_sprite(SPRITE_ID_STARTER_PREVIEW, offset_x, offset_y, sequence);
        }
    } else if select.preview_sprite.loading {
        let loading = Paragraph::new("[Loading...]")
            .style(Style::default().fg(TEXT_DIM))
            .alignment(Alignment::Center);
        frame.render_widget(loading, preview_layout[0]);
    }

    // Stats
    if let Some(info) = select.preview_info.as_ref() {
        let stats_lines = vec![
            Line::from(Span::styled(
                format_name(&info.name).to_ascii_uppercase(),
                Style::default()
                    .fg(ACCENT_GREEN)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(format!(
                "HP: {:>3}  ATK: {:>3}  DEF: {:>3}",
                info.hp, info.attack, info.defense
            )),
            Line::from(format!(
                "SpA:{:>3}  SpD:{:>3}  SPD:{:>3}",
                info.sp_attack, info.sp_defense, info.speed
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Z/Enter: Choose this Pokemon!",
                Style::default().fg(ACCENT_GOLD),
            )),
        ];
        let stats_para = Paragraph::new(Text::from(stats_lines))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(stats_para, preview_layout[1]);
    }
}

fn dim_background(frame: &mut Frame, area: Rect) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &mut buf[(x, y)];
            if let Color::Rgb(r, g, b) = cell.bg {
                cell.bg = Color::Rgb(r / 2, g / 2, b / 2);
            }
            if let Color::Rgb(r, g, b) = cell.fg {
                cell.fg = Color::Rgb(r / 2, g / 2, b / 2);
            }
        }
    }
}

fn fill_area(frame: &mut Frame, area: Rect, bg: Color, fg: Color) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf[(x, y)].set_char(' ').set_bg(bg).set_fg(fg);
        }
    }
}

fn render_pause_menu(frame: &mut Frame, area: Rect, state: &AppState) {
    // Clear sprites so they don't show through the modal
    sprite_backend::clear_sprites();

    // Dim the background
    dim_background(frame, area);

    // Draw modal in center
    let modal_width = 24;
    let modal_height = 10;
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the modal area first (fill with spaces and background color)
    fill_area(frame, modal_area, BG_PANEL, TEXT_MAIN);

    let block = panel_block(" PAUSED ", BG_PANEL);
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let options = ["Resume", "Save Game", "Quit to Menu"];
    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (idx, label) in options.iter().enumerate() {
        lines.push(menu_line(label, idx == state.pause_menu.selected));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "ESC: Close",
        Style::default().fg(TEXT_DIM),
    )));

    let paragraph = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn render_message_modal(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(message) = state.message.as_deref() else {
        return;
    };

    // Clear sprites so they don't show through the modal
    sprite_backend::clear_sprites();

    // Dim the background
    dim_background(frame, area);

    let modal_width = area.width.min(70).saturating_sub(4).max(28);
    let modal_height = area.height.min(9).saturating_sub(4).max(5);
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    fill_area(frame, modal_area, BG_PANEL, TEXT_MAIN);

    let title = if state.mode == GameMode::Battle {
        " BATTLE "
    } else {
        " MESSAGE "
    };
    let block = panel_block(title, BG_PANEL);
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let lines = vec![
        Line::from(Span::styled(message, Style::default().fg(TEXT_MAIN))),
        Line::from(""),
        Line::from(Span::styled(
            "Enter/Z: Continue",
            Style::default().fg(TEXT_DIM),
        )),
    ];
    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(TEXT_MAIN))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, inner);
}

fn render_battle_message_modal(frame: &mut Frame, area: Rect, message: &str) {
    if message.is_empty() {
        return;
    }

    // Clear sprites so they don't show through the modal
    sprite_backend::clear_sprites();

    dim_background(frame, area);

    let modal_width = area.width.min(72).saturating_sub(6).max(30);
    let modal_height = area.height.min(9).saturating_sub(4).max(5);
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    fill_area(frame, modal_area, BG_PANEL, TEXT_MAIN);

    let block = panel_block(" BATTLE ", BG_PANEL);
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let lines = vec![
        Line::from(Span::styled(message, Style::default().fg(TEXT_MAIN))),
        Line::from(""),
        Line::from(Span::styled(
            "Enter/Z: Continue",
            Style::default().fg(TEXT_DIM),
        )),
    ];
    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(TEXT_MAIN))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, inner);
}

fn render_overworld(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.width < 30 || area.height < 16 {
        let warning = Paragraph::new("Terminal too small - expand window.")
            .style(Style::default().fg(TEXT_DIM))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(warning, area);
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(7),
        ])
        .split(area);

    render_overworld_header(frame, layout[0], state);
    let show_hud = area.width >= 90;
    render_overworld_body(frame, layout[1], state, show_hud);
    render_overworld_status(frame, layout[2], state, show_hud);
}

fn render_overworld_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block(" ROUTE ", BG_HEADER);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let title = state.map.name.to_ascii_uppercase();
    let player = format_name(&state.player_name());
    let level = state.active_level();
    let line = Line::from(vec![
        Span::styled(
            title,
            Style::default()
                .fg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  •  "),
        Span::styled(
            format!("Partner {} Lv {}", player, level),
            Style::default().fg(TEXT_MAIN),
        ),
        Span::raw("  •  "),
        Span::styled(
            format!("Steps {}", state.player.steps),
            Style::default().fg(TEXT_DIM),
        ),
    ]);
    let paragraph = Paragraph::new(Text::from(vec![line]))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn render_overworld_body(frame: &mut Frame, area: Rect, state: &AppState, show_hud: bool) {
    if show_hud {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(26)])
            .split(area);
        render_map(frame, columns[0], state);
        render_overworld_panel(frame, columns[1], state);
    } else {
        render_map(frame, area, state);
    }
}

fn render_overworld_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block("HUD", BG_PANEL_ALT);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
        .split(inner);

    let hp_max = state.player_max_hp();
    let hp_current = state
        .active_member()
        .map(|member| member.hp)
        .unwrap_or(state.player_max_hp())
        .min(hp_max);
    let exp_current = state.exp_progress();
    let exp_next = state.exp_to_next_level().max(1);
    let party_count = state.party.len();
    let balls = pokeball_count(state);
    let bag_summary = bag_summary(state);
    let lines = vec![
        Line::from(Span::styled(
            format_name(&state.player_name()),
            Style::default()
                .fg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Lv {}", state.active_level())),
        Line::from(""),
        meter_line("HP", hp_current as u32, hp_max as u32, 12, ACCENT_GREEN),
        meter_line("EXP", exp_current, exp_next, 12, ACCENT_GOLD),
        Line::from(""),
        Line::from(Span::styled(
            format!("Party: {}/3", party_count),
            Style::default().fg(TEXT_MAIN),
        )),
        Line::from(Span::styled(
            format!("Balls: x{}", balls),
            Style::default().fg(TEXT_MAIN),
        )),
        Line::from(Span::styled(
            format!("Bag: {}", bag_summary),
            Style::default().fg(TEXT_DIM),
        )),
        if state.boss_defeated {
            Line::from(Span::styled(
                "Demo complete!",
                Style::default()
                    .fg(ACCENT_GOLD)
                    .add_modifier(Modifier::BOLD),
            ))
        } else if state.has_relic {
            Line::from(Span::styled(
                "Relic acquired",
                Style::default().fg(ACCENT_GOLD),
            ))
        } else {
            Line::from("")
        },
    ];
    let paragraph = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, layout[0]);
    render_party_sprite_strip(
        frame,
        layout[1],
        state,
        BG_PANEL_ALT,
        SPRITE_ID_PARTY_BASE,
        0.55,
        1.0,
        false,
    );
}

fn render_map(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block(state.map.name.as_str(), BG_PANEL_ALT);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 8 || inner.height < 4 {
        let warning = Paragraph::new("Resize for map view.")
            .style(Style::default().fg(TEXT_DIM))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(warning, inner);
        return;
    }

    let render = map_renderer().render_base(
        frame,
        inner,
        &state.map,
        Camera {
            focus_x: state.player.x,
            focus_y: state.player.y,
        },
        true,
    );
    if render.view_tiles_h == 0 || render.view_tiles_v == 0 {
        return;
    }

    let buf = frame.buffer_mut();
    for pickup in &state.pickups {
        if pickup.x == state.player.x && pickup.y == state.player.y {
            continue;
        }
        if let Some((center_x, center_y)) = render.marker_cell(pickup.x, pickup.y) {
            if let Some(cell) = buf.cell_mut((center_x, center_y)) {
                cell.set_fg(ACCENT_GOLD).set_char('*');
            }
        }
    }

    let player_sprite = match state.player.facing {
        MoveDir::Right => state
            .player_sprite
            .sprite_flipped
            .as_ref()
            .or(state.player_sprite.sprite.as_ref()),
        _ => state.player_sprite.sprite.as_ref(),
    };
    if let Some(sprite) = player_sprite {
        if let Some((tile_x, tile_y)) = render.tile_cell_origin(state.player.x, state.player.y) {
            let (cols, rows) = sprite_fit(sprite, render.cols_per_tile, render.rows_per_tile);
            let sprite_frame = sprite.frame(state.player_sprite.frame_index);
            if let Ok(sequence) = sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_PLAYER_MAP)
            {
                let offset_x = tile_x + render.cols_per_tile.saturating_sub(cols) / 2;
                let offset_y = tile_y + render.rows_per_tile.saturating_sub(rows) / 2;
                sprite_backend::set_sprite(SPRITE_ID_PLAYER_MAP, offset_x, offset_y, sequence);
            }
        }
    }
}

fn render_overworld_status(frame: &mut Frame, area: Rect, state: &AppState, show_hud: bool) {
    let block = panel_block("STATUS", BG_PANEL_ALT);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let message = state
        .message
        .as_deref()
        .unwrap_or("Wander the grass to find Pokemon.");
    if show_hud {
        let lines = vec![
            Line::from(Span::styled(message, Style::default().fg(TEXT_MAIN))),
            Line::from(Span::styled(
                "Arrows/WASD move  |  Esc menu",
                Style::default().fg(TEXT_DIM),
            )),
        ];
        let paragraph = Paragraph::new(Text::from(lines))
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    } else {
        let player = format_name(&state.player_name());
        let hp_max = state.player_max_hp();
        let hp_current = state
            .active_member()
            .map(|member| member.hp)
            .unwrap_or(hp_max)
            .min(hp_max);
        let exp_current = state.exp_progress();
        let exp_next = state.exp_to_next_level().max(1);
        let bag_summary = bag_summary(state);
        let party_count = state.party.len();
        let balls = pokeball_count(state);
        let bar_width = if area.width >= 80 { 26 } else { 16 };
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(6), Constraint::Length(4)])
            .split(inner);
        let lines = vec![
            Line::from(format!("Partner {}  Lv {}", player, state.active_level())),
            meter_line(
                "HP",
                hp_current as u32,
                hp_max as u32,
                bar_width,
                ACCENT_GREEN,
            ),
            meter_line("EXP", exp_current, exp_next, bar_width, ACCENT_GOLD),
            Line::from(format!(
                "Steps {}  |  Party {}/3  |  Balls x{}",
                state.player.steps, party_count, balls
            )),
            Line::from(format!("Bag {}", bag_summary)),
            if state.boss_defeated {
                Line::from(Span::styled(
                    "Demo complete!",
                    Style::default()
                        .fg(ACCENT_GOLD)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if state.has_relic {
                Line::from(Span::styled(
                    "Relic acquired",
                    Style::default().fg(ACCENT_GOLD),
                ))
            } else {
                Line::from("")
            },
            Line::from(Span::styled(message, Style::default().fg(TEXT_MAIN))),
            Line::from(Span::styled(
                "Arrows/WASD move  |  Esc menu",
                Style::default().fg(TEXT_DIM),
            )),
        ];
        let paragraph = Paragraph::new(Text::from(lines))
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, layout[0]);
        render_party_sprite_strip(
            frame,
            layout[1],
            state,
            BG_PANEL_ALT,
            SPRITE_ID_PARTY_BASE + 10,
            0.55,
            1.0,
            false,
        );
    }
}

fn render_battle(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().style(Style::default().bg(BG_BASE));
    frame.render_widget(block, area);

    // Command box is fixed at bottom, pokemon panels split the rest
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // Pokemon area (flexible)
            Constraint::Length(7), // Command box (fixed)
        ])
        .split(area);

    // Split pokemon area into enemy (top) and player (bottom)
    let pokemon_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);

    render_enemy_panel(frame, pokemon_layout[0], state);
    render_player_panel(frame, pokemon_layout[1], state);
    render_battle_command(frame, layout[1], state);

    if let Some(battle) = state.battle.as_ref().filter(|battle| battle_should_show_modal(battle)) {
        render_battle_message_modal(frame, area, &battle.message);
    }
}

fn render_enemy_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let enemy_name = state
        .battle
        .as_ref()
        .map(|battle| format_name(&battle.enemy_name))
        .unwrap_or_else(|| "Enemy".to_string());
    let is_boss = state
        .battle
        .as_ref()
        .map(|battle| battle.kind == crate::state::BattleKind::Boss)
        .unwrap_or(false);
    let title = if is_boss {
        format!(" BOSS {} ", enemy_name.to_ascii_uppercase())
    } else {
        format!(" WILD {} ", enemy_name.to_ascii_uppercase())
    };
    let block = panel_block(title.as_str(), BG_PANEL_ALT);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Stats on left (narrow), sprite on right (wide)
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(10)])
        .split(inner);
    render_enemy_stats(frame, layout[0], state);
    render_enemy_sprite(frame, layout[1], state);
}

fn render_enemy_stats(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(battle) = state.battle.as_ref() else {
        return;
    };
    let bar_width = area.width.saturating_sub(6).min(16).max(8) as usize;
    let lines = vec![
        hp_line_scaled(battle.enemy_hp, battle.enemy_hp_max, bar_width),
        Line::from(Span::styled(
            format!("Lv {}", battle.enemy_level),
            Style::default().fg(TEXT_DIM),
        )),
    ];
    let paragraph = Paragraph::new(Text::from(lines)).style(Style::default().fg(TEXT_MAIN));
    frame.render_widget(paragraph, area);
}

fn render_enemy_sprite(frame: &mut Frame, area: Rect, state: &AppState) {
    if state
        .battle
        .as_ref()
        .map(|battle| battle.captured)
        .unwrap_or(false)
    {
        return;
    }
    if let Some(sprite) = state.enemy_sprite.sprite.as_ref() {
        let (cols, rows) = sprite_fit(sprite, area.width, area.height.saturating_sub(1));
        let sprite_frame = sprite.frame(state.enemy_sprite.frame_index);
        if let Ok(sequence) =
            sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_ENEMY_BATTLE)
        {
            // Center horizontally, align to bottom
            let offset_x = area.x.saturating_add(area.width.saturating_sub(cols) / 2);
            let offset_y = area.y.saturating_add(area.height.saturating_sub(rows));
            sprite_backend::set_sprite(SPRITE_ID_ENEMY_BATTLE, offset_x, offset_y, sequence);
            return;
        }
    }

    let content = if state.enemy_sprite.loading {
        "[loading]"
    } else {
        "[no sprite]"
    };
    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(TEXT_DIM))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_player_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let player_name = format_name(&state.player_name());
    let title = format!(" {} ", player_name.to_ascii_uppercase());
    let block = panel_block(title.as_str(), BG_PANEL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(22)])
        .split(inner);
    render_party_sprite_strip(
        frame,
        layout[0],
        state,
        BG_PANEL,
        SPRITE_ID_PARTY_BASE + 20,
        0.88,
        1.12,
        true,
    );
    render_player_stats(frame, layout[1], state);
}

fn render_player_stats(frame: &mut Frame, area: Rect, state: &AppState) {
    let (current, max) = state
        .battle
        .as_ref()
        .map(|battle| (battle.player_hp, battle.player_hp_max))
        .unwrap_or((state.player_max_hp(), state.player_max_hp()));
    let bar_width = area.width.saturating_sub(6).min(16).max(8) as usize;
    let mut lines = vec![
        hp_line_scaled(current, max, bar_width),
        Line::from(Span::styled(
            format!("Lv {}", state.active_level()),
            Style::default().fg(TEXT_DIM),
        )),
    ];
    if let Some(member) = state.active_member() {
        if member.ability_cd > 0 {
            lines.push(Line::from(Span::styled(
                format!("Ability CD: {}", member.ability_cd),
                Style::default().fg(TEXT_DIM),
            )));
        }
    }
    if let Some(battle) = state.battle.as_ref() {
        if battle.guard_turns > 0 && battle.guard_pct > 0 {
            lines.push(Line::from(Span::styled(
                format!("Guard -{}% ({}t)", battle.guard_pct, battle.guard_turns),
                Style::default().fg(ACCENT_GOLD),
            )));
        }
    }
    let paragraph = Paragraph::new(Text::from(lines)).style(Style::default().fg(TEXT_MAIN));
    frame.render_widget(paragraph, area);
}

fn render_battle_command(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = panel_block("COMMAND", BG_PANEL_ALT);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(battle) = state.battle.as_ref() else {
        return;
    };

    let is_compact = inner.width < 50;
    let sections = if is_compact {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(inner)
    };

    render_battle_prompt(frame, sections[0], battle);
    render_battle_actions(frame, sections[1], state, battle);
}

fn render_battle_prompt(frame: &mut Frame, area: Rect, battle: &crate::state::BattleState) {
    let mut lines = Vec::new();
    if matches!(battle.stage, BattleStage::Menu | BattleStage::ItemMenu) {
        lines.push(Line::from(Span::styled(
            battle.message.clone(),
            Style::default().fg(TEXT_MAIN),
        )));
        lines.push(Line::from(Span::styled(
            "Arrows/WASD: Navigate",
            Style::default().fg(TEXT_DIM),
        )));
        if battle.stage == BattleStage::ItemMenu {
            lines.push(Line::from(Span::styled(
                "Z/Enter: Use  |  Esc: Back",
                Style::default().fg(TEXT_DIM),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "Z/Enter: Select",
                Style::default().fg(TEXT_DIM),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Resolving turn...",
            Style::default().fg(TEXT_DIM),
        )));
        lines.push(Line::from(Span::styled(
            "Enter/Z: Continue",
            Style::default().fg(TEXT_DIM),
        )));
    }
    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(TEXT_MAIN))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_battle_actions(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    battle: &crate::state::BattleState,
) {
    let lines = match battle.stage {
        BattleStage::Menu => battle_menu_lines(battle.menu_index, battle.kind),
        BattleStage::ItemMenu => battle_item_lines(state, battle.item_index),
        _ => vec![Line::from(Span::styled(
            "Enter/Z: Continue",
            Style::default().fg(TEXT_DIM),
        ))],
    };
    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(TEXT_MAIN))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn battle_menu_lines(selected: usize, kind: BattleKind) -> Vec<Line<'static>> {
    let options = ["FIGHT", "BAG", "CATCH", "ABILITY", "RUN"];
    let mut lines = Vec::new();
    for (idx, label) in options.iter().enumerate() {
        let disabled = kind == BattleKind::Boss && (idx == 2 || idx == 4);
        let style = if idx == selected {
            if disabled {
                Style::default()
                    .fg(TEXT_DIM)
                    .bg(adjust_color(BG_PANEL_ALT, 10))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(HIGHLIGHT_TEXT)
                    .bg(HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD)
            }
        } else if disabled {
            Style::default().fg(TEXT_DIM)
        } else {
            Style::default().fg(TEXT_MAIN)
        };
        lines.push(Line::from(Span::styled(label.to_string(), style)));
    }
    lines
}

fn battle_should_show_modal(battle: &crate::state::BattleState) -> bool {
    !matches!(battle.stage, BattleStage::Menu | BattleStage::ItemMenu)
}

fn hp_line_scaled(current: u16, max: u16, width: usize) -> Line<'static> {
    let width = width.max(6);
    let ratio = if max == 0 {
        0.0
    } else {
        current as f32 / max as f32
    };
    let filled = ((ratio * width as f32).round() as usize).min(width);
    let empty = width.saturating_sub(filled);
    let filled_bar = "█".repeat(filled);
    let empty_bar = "░".repeat(empty);
    let color = if ratio > 0.5 {
        ACCENT_GREEN
    } else if ratio > 0.2 {
        ACCENT_GOLD
    } else {
        Color::Rgb(220, 96, 96)
    };
    let bar_bg = Color::Rgb(24, 32, 24);
    let bar_bg_dim = Color::Rgb(20, 26, 20);
    Line::from(vec![
        Span::raw("HP "),
        Span::styled(
            filled_bar,
            Style::default()
                .fg(color)
                .bg(bar_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(empty_bar, Style::default().fg(TEXT_DIM).bg(bar_bg_dim)),
        Span::raw(format!(" {}/{}", current, max)),
    ])
}

fn sprite_fit(sprite: &sprite::SpriteData, max_cols: u16, max_rows: u16) -> (u16, u16) {
    if max_cols == 0 || max_rows == 0 || sprite.height == 0 {
        return (max_cols, max_rows);
    }
    let image_ratio = sprite.width as f32 / sprite.height as f32;
    let max_cols_f = max_cols as f32;
    let max_rows_f = max_rows as f32;
    let cols_for_max_rows = image_ratio * max_rows_f * CELL_ASPECT;
    if cols_for_max_rows <= max_cols_f {
        let cols = cols_for_max_rows.max(1.0).round() as u16;
        return (cols.max(1), max_rows.max(1));
    }
    let rows_for_max_cols = max_cols_f / (image_ratio * CELL_ASPECT);
    let rows = rows_for_max_cols.max(1.0).round() as u16;
    (max_cols.max(1), rows.min(max_rows).max(1))
}

fn sprite_fit_scaled(
    sprite: &sprite::SpriteData,
    max_cols: u16,
    max_rows: u16,
    scale: f32,
) -> (u16, u16) {
    let scale = scale.clamp(0.1, 1.0);
    let cols = ((max_cols as f32) * scale).floor().max(1.0) as u16;
    let rows = ((max_rows as f32) * scale).floor().max(1.0) as u16;
    sprite_fit(sprite, cols, rows)
}

fn panel_block<'a, T>(title: T, bg: Color) -> Block<'a>
where
    T: Into<Title<'a>>,
{
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .style(Style::default().bg(bg).fg(TEXT_MAIN))
        .border_style(Style::default().fg(BORDER_ACCENT))
}

fn meter_line(label: &str, current: u32, max: u32, width: usize, color: Color) -> Line<'static> {
    let max = max.max(1);
    let ratio = current as f32 / max as f32;
    let filled = ((ratio * width as f32).round() as usize).min(width);
    let empty = width.saturating_sub(filled);
    let filled_bar = "█".repeat(filled);
    let empty_bar = "░".repeat(empty);
    Line::from(vec![
        Span::styled(format!("{label} "), Style::default().fg(TEXT_DIM)),
        Span::styled(
            filled_bar,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(empty_bar, Style::default().fg(TEXT_DIM)),
        Span::styled(format!(" {current}/{max}"), Style::default().fg(TEXT_DIM)),
    ])
}

fn menu_line(label: &str, selected: bool) -> Line<'static> {
    let style = if selected {
        Style::default()
            .fg(HIGHLIGHT_TEXT)
            .bg(HIGHLIGHT_BG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT_MAIN)
    };
    Line::from(Span::styled(label.to_string(), style))
}

fn battle_item_lines(state: &AppState, selected: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut entries: Vec<(String, bool)> = Vec::new();
    for (idx, stack) in state
        .inventory
        .iter()
        .filter(|stack| stack.qty > 0)
        .filter(|stack| {
            matches!(
                stack.kind,
                crate::state::ItemKind::Potion | crate::state::ItemKind::SuperPotion
            )
        })
        .enumerate()
    {
        let label = format!("{} x{}", stack.kind.label(), stack.qty);
        entries.push((label, idx == selected));
    }
    if entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "Bag is empty.",
            Style::default().fg(TEXT_DIM),
        )));
        return lines;
    }
    for (label, is_selected) in entries {
        lines.push(menu_line(&label, is_selected));
    }
    lines
}

fn bag_summary(state: &AppState) -> String {
    let mut items = Vec::new();
    for stack in &state.inventory {
        if stack.qty > 0 && stack.kind != crate::state::ItemKind::PokeBall {
            items.push(format!("{} x{}", stack.kind.label(), stack.qty));
        }
    }
    if items.is_empty() {
        "empty".to_string()
    } else {
        items.join(", ")
    }
}

fn render_party_sprite_strip(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    bg: Color,
    sprite_base: u32,
    scale: f32,
    active_boost: f32,
    show_shadow: bool,
) {
    if area.width == 0 || area.height == 0 || state.party.is_empty() {
        return;
    }
    let buf = frame.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(bg).set_fg(bg).set_char(' ');
            }
        }
    }
    let slots = state.party.len().max(1);
    let slot_width = (area.width / slots as u16).max(1);
    for (idx, member) in state.party.iter().enumerate() {
        let slot_x = area.x + (idx as u16 * slot_width);
        let width = if idx + 1 == slots {
            area.x.saturating_add(area.width).saturating_sub(slot_x)
        } else {
            slot_width
        };
        let slot = Rect::new(slot_x, area.y, width.max(1), area.height);
        let is_active = idx == state.active_party_index;
        let mut scale = scale;
        if is_active {
            scale = (scale * active_boost).clamp(0.1, 1.2);
        }
        let sprite_state = state.party_sprites.get(idx);
        let draw_hp_bar = show_shadow || sprite_state.and_then(|state| state.sprite.as_ref()).is_none();
        let can_draw_shadow = is_active && show_shadow && slot.height >= 2;
        let reserved_rows = (if draw_hp_bar { 1 } else { 0 }) + (if can_draw_shadow { 1 } else { 0 });
        let sprite_height = slot.height.saturating_sub(reserved_rows as u16);
        let sprite_area = Rect::new(slot.x, slot.y, slot.width, sprite_height.max(1));
        let mut sprite_drawn = false;

        if let Some(sprite_state) = sprite_state {
            if let Some(sprite) = sprite_state.sprite.as_ref() {
            if sprite_height > 0 {
                let (cols, rows) =
                    sprite_fit_scaled(sprite, sprite_area.width, sprite_area.height, scale);
                let frame_data = sprite.frame(sprite_state.frame_index);
                if let Ok(sequence) =
                    sprite::kitty_sequence(frame_data, cols, rows, sprite_base + idx as u32)
                {
                    let offset_x = sprite_area
                        .x
                        .saturating_add(sprite_area.width.saturating_sub(cols) / 2);
                    let offset_y = sprite_area
                        .y
                        .saturating_add(sprite_area.height.saturating_sub(rows) / 2);
                    sprite_backend::set_sprite(
                        sprite_base + idx as u32,
                        offset_x,
                        offset_y,
                        sequence,
                    );
                    sprite_drawn = true;
                }
            }
            }
        }

        if can_draw_shadow {
            let shadow_color = adjust_color(bg, -20);
            let shadow_width = (slot.width as f32 * 0.6).round().max(1.0) as u16;
            let shadow_x = slot
                .x
                .saturating_add(slot.width.saturating_sub(shadow_width) / 2);
            let shadow_y = slot.y.saturating_add(slot.height.saturating_sub(1));
            for x in shadow_x..shadow_x.saturating_add(shadow_width) {
                if let Some(cell) = buf.cell_mut((x, shadow_y)) {
                    cell.set_bg(shadow_color).set_fg(shadow_color).set_char('▄');
                }
            }
        }

        let bar_y = if draw_hp_bar && slot.height > 0 {
            let mut y = slot.y.saturating_add(slot.height.saturating_sub(1));
            if can_draw_shadow && y > slot.y {
                y = y.saturating_sub(1);
            }
            Some(y)
        } else {
            None
        };

        if let Some(bar_y) = bar_y {
            let max_hp = calc_hp(member.info.hp, member.level).max(1);
            let ratio = (member.hp.min(max_hp) as f32) / (max_hp as f32);
            let bar_width = slot
                .width
                .saturating_sub(2)
                .min(12)
                .max(4)
                .min(slot.width);
            let start_x = slot
                .x
                .saturating_add(slot.width.saturating_sub(bar_width) / 2);
            let filled = ((ratio * bar_width as f32).round() as u16).min(bar_width);
            let color = if ratio > 0.5 {
                ACCENT_GREEN
            } else if ratio > 0.2 {
                ACCENT_GOLD
            } else {
                Color::Rgb(220, 96, 96)
            };
            for i in 0..bar_width {
                let (ch, fg) = if i < filled {
                    ('█', color)
                } else {
                    ('░', TEXT_DIM)
                };
                if let Some(cell) = buf.cell_mut((start_x + i, bar_y)) {
                    cell.set_bg(bg).set_fg(fg).set_char(ch);
                }
            }
        }
        if !sprite_drawn {
            let label = member
                .info
                .name
                .chars()
                .next()
                .map(|ch| ch.to_ascii_uppercase())
                .unwrap_or('?');
            let cx = sprite_area.x + sprite_area.width / 2;
            let cy = sprite_area.y + sprite_area.height / 2;
            if let Some(cell) = buf.cell_mut((cx, cy)) {
                cell.set_bg(bg).set_fg(TEXT_DIM).set_char(label);
            }
        }
    }
}

fn pokeball_count(state: &AppState) -> u16 {
    state
        .inventory
        .iter()
        .find(|stack| stack.kind == crate::state::ItemKind::PokeBall)
        .map(|stack| stack.qty)
        .unwrap_or(0)
}

fn format_name(name: &str) -> String {
    name.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let rest = chars.as_str();
                    format!("{}{}", first.to_ascii_uppercase(), rest)
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
