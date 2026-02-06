use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use tui_dispatch::{EventKind, EventOutcome, RenderContext};

use crate::action::Action;
use crate::sprite;
use crate::sprite_backend;
use crate::state::{AppState, BattleStage, Direction as MoveDir, GameMode, Tile};

const BG_BASE: Color = Color::Rgb(24, 36, 26);
const BG_PANEL: Color = Color::Rgb(34, 58, 38);
const BG_PANEL_ALT: Color = Color::Rgb(28, 48, 32);
const TEXT_MAIN: Color = Color::Rgb(228, 236, 214);
const TEXT_DIM: Color = Color::Rgb(172, 186, 160);
const ACCENT_GREEN: Color = Color::Rgb(104, 204, 120);
const ACCENT_GOLD: Color = Color::Rgb(222, 196, 120);
const CELL_ASPECT: f32 = 2.0;
const MAP_TILES_V: u16 = 9;

const SPRITE_ID_PLAYER_MAP: u32 = 2;
const SPRITE_ID_ENEMY_BATTLE: u32 = 3;
const SPRITE_ID_PLAYER_BATTLE: u32 = 4;

// Tile colors
const TILE_GRASS: Color = Color::Rgb(34, 112, 58);
const TILE_GRASS_ALT: Color = Color::Rgb(42, 128, 68);
const TILE_PATH: Color = Color::Rgb(156, 132, 76);
const TILE_PATH_ALT: Color = Color::Rgb(146, 122, 66);
const TILE_SAND: Color = Color::Rgb(194, 178, 128);
const TILE_SAND_ALT: Color = Color::Rgb(184, 168, 118);
const TILE_WALL: Color = Color::Rgb(66, 74, 66);
const TILE_WALL_ALT: Color = Color::Rgb(56, 64, 56);
const TILE_WATER: Color = Color::Rgb(48, 86, 146);
const TILE_WATER_ALT: Color = Color::Rgb(58, 96, 156);
const TILE_BORDER: Color = Color::Rgb(18, 24, 18);

fn tile_colors(tile: Tile) -> (Color, Color, char, char) {
    match tile {
        Tile::Grass => (TILE_GRASS, TILE_GRASS_ALT, '"', '\''),
        Tile::Path => (TILE_PATH, TILE_PATH_ALT, '.', ' '),
        Tile::Sand => (TILE_SAND, TILE_SAND_ALT, ':', '.'),
        Tile::Wall => (TILE_WALL, TILE_WALL_ALT, '#', '#'),
        Tile::Water => (TILE_WATER, TILE_WATER_ALT, '~', '≈'),
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
    sprite_backend::clear_sprites();
    frame.render_widget(Block::default().style(Style::default().bg(BG_BASE)), area);
    match state.mode {
        GameMode::Overworld => render_overworld(frame, area, state),
        GameMode::Battle => render_battle(frame, area, state),
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
    if matches!(key.code, KeyCode::Char('q')) {
        return EventOutcome::action(Action::Quit);
    }
    match state.mode {
        GameMode::Overworld => handle_overworld_key(key),
        GameMode::Battle => handle_battle_key(key, state),
    }
}

fn handle_overworld_key(key: KeyEvent) -> EventOutcome<Action> {
    let action = match key.code {
        KeyCode::Up | KeyCode::Char('w') => Some(Action::Move(MoveDir::Up)),
        KeyCode::Down | KeyCode::Char('s') => Some(Action::Move(MoveDir::Down)),
        KeyCode::Left | KeyCode::Char('a') => Some(Action::Move(MoveDir::Left)),
        KeyCode::Right | KeyCode::Char('d') => Some(Action::Move(MoveDir::Right)),
        _ => None,
    };
    EventOutcome::from(action)
}

fn handle_battle_key(key: KeyEvent, state: &AppState) -> EventOutcome<Action> {
    if matches!(key.code, KeyCode::Enter | KeyCode::Char('z') | KeyCode::Char('Z')) {
        return EventOutcome::action(Action::BattleConfirm);
    }
    if let Some(battle) = state.battle.as_ref() {
        if battle.stage == BattleStage::Menu {
            let action = match key.code {
                KeyCode::Up | KeyCode::Left => Some(Action::BattleMenuPrev),
                KeyCode::Down | KeyCode::Right => Some(Action::BattleMenuNext),
                _ => None,
            };
            return EventOutcome::from(action);
        }
    }
    EventOutcome::ignored()
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
        .constraints([Constraint::Min(12), Constraint::Length(6)])
        .split(area);

    render_map(frame, layout[0], state);
    render_overworld_status(frame, layout[1], state);
}

fn render_map(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(state.map.name.as_str())
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN));
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

    // Each tile spans multiple terminal cells for a zoomed-in view
    // Reserve 1 cell for borders between tiles
    let rows_per_tile = ((inner.height / MAP_TILES_V).max(3)).saturating_sub(1);
    let cols_per_tile = ((rows_per_tile as f32 * CELL_ASPECT).round() as u16).max(2);

    // Total cells per tile including border
    let tile_stride_v = rows_per_tile + 1;
    let tile_stride_h = cols_per_tile + 1;

    let view_tiles_h = (inner.width / tile_stride_h).min(state.map.width);
    let view_tiles_v = (inner.height / tile_stride_v).min(state.map.height);
    if view_tiles_h == 0 || view_tiles_v == 0 {
        return;
    }

    let used_cols = view_tiles_h * tile_stride_h;
    let used_rows = view_tiles_v * tile_stride_v;
    let pad_x = (inner.width.saturating_sub(used_cols)) / 2;
    let pad_y = (inner.height.saturating_sub(used_rows)) / 2;
    let origin_x = inner.x + pad_x;
    let origin_y = inner.y + pad_y;

    let (start_x, start_y) = map_viewport(state, view_tiles_h, view_tiles_v);
    let buf = frame.buffer_mut();

    // Draw tiles with borders
    for tile_row in 0..view_tiles_v {
        for tile_col in 0..view_tiles_h {
            let map_x = start_x + tile_col;
            let map_y = start_y + tile_row;
            let tile = state.map.tile(map_x, map_y);
            let (color_main, color_alt, char_main, char_alt) = tile_colors(tile);

            let cell_x = origin_x + tile_col * tile_stride_h;
            let cell_y = origin_y + tile_row * tile_stride_v;

            // Draw the tile interior with subtle pattern
            for dy in 0..rows_per_tile {
                for dx in 0..cols_per_tile {
                    let x = cell_x + dx;
                    let y = cell_y + dy;
                    if x < inner.x + inner.width && y < inner.y + inner.height {
                        // Checkerboard pattern for visual interest
                        let checker = ((dx + dy) % 2) == 0;
                        let bg = if checker { color_main } else { color_alt };
                        let ch = if checker { char_main } else { char_alt };
                        buf[(x, y)].set_bg(bg).set_fg(TEXT_DIM).set_char(ch);
                    }
                }
            }

            // Draw right border (vertical line)
            let border_x = cell_x + cols_per_tile;
            if border_x < inner.x + inner.width {
                for dy in 0..rows_per_tile {
                    let y = cell_y + dy;
                    if y < inner.y + inner.height {
                        buf[(border_x, y)]
                            .set_bg(TILE_BORDER)
                            .set_fg(BG_BASE)
                            .set_char('│');
                    }
                }
            }

            // Draw bottom border (horizontal line)
            let border_y = cell_y + rows_per_tile;
            if border_y < inner.y + inner.height {
                for dx in 0..cols_per_tile {
                    let x = cell_x + dx;
                    if x < inner.x + inner.width {
                        buf[(x, border_y)]
                            .set_bg(TILE_BORDER)
                            .set_fg(BG_BASE)
                            .set_char('─');
                    }
                }
                // Draw corner/intersection
                if border_x < inner.x + inner.width {
                    buf[(border_x, border_y)]
                        .set_bg(TILE_BORDER)
                        .set_fg(BG_BASE)
                        .set_char('┼');
                }
            }
        }
    }

    // Draw player sprite on top
    if let Some(sprite) = state.player_sprite.sprite.as_ref() {
        let player_col = state.player.x.saturating_sub(start_x);
        let player_row = state.player.y.saturating_sub(start_y);
        if player_col < view_tiles_h && player_row < view_tiles_v {
            let (cols, rows) = sprite_fit(sprite, cols_per_tile, rows_per_tile);
            let sprite_frame = sprite.frame(state.player_sprite.frame_index);
            if let Ok(sequence) =
                sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_PLAYER_MAP)
            {
                let tile_x = origin_x + player_col * tile_stride_h;
                let tile_y = origin_y + player_row * tile_stride_v;
                let offset_x = tile_x + cols_per_tile.saturating_sub(cols) / 2;
                let offset_y = tile_y + rows_per_tile.saturating_sub(rows) / 2;
                sprite_backend::set_sprite(SPRITE_ID_PLAYER_MAP, offset_x, offset_y, sequence);
            }
        }
    }
}

fn render_overworld_status(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("STATUS")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let message = state
        .message
        .as_deref()
        .unwrap_or("Wander the grass to find Pokemon.");
    let player = format_name(&state.player_name());
    let lines = vec![
        Line::from(format!(
            "Trainer: {}   Steps: {}",
            player, state.player.steps
        )),
        Line::from(message),
        Line::from(Span::styled(
            "Arrows/WASD move  |  q quit",
            Style::default().fg(TEXT_DIM),
        )),
    ];
    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(TEXT_MAIN))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn render_battle(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().style(Style::default().bg(BG_BASE));
    frame.render_widget(block, area);

    // Command box is fixed at bottom, pokemon panels split the rest
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // Pokemon area (flexible)
            Constraint::Length(6), // Command box (fixed)
        ])
        .split(area);

    // Split pokemon area into enemy (top) and player (bottom)
    let pokemon_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);

    render_enemy_panel(frame, pokemon_layout[0], state);
    render_player_panel(frame, pokemon_layout[1], state);
    render_battle_text(frame, layout[1], state);
}

fn render_enemy_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let enemy_name = state
        .battle
        .as_ref()
        .map(|battle| format_name(&battle.enemy_name))
        .unwrap_or_else(|| "Enemy".to_string());
    let title = format!(" WILD {} ", enemy_name.to_ascii_uppercase());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN));
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
    let lines = vec![
        hp_line(battle.enemy_hp, battle.enemy_hp_max),
        Line::from(Span::styled(
            format!("Lv {}", 5 + (battle.enemy_hp_max / 10)),
            Style::default().fg(TEXT_DIM),
        )),
    ];
    let paragraph = Paragraph::new(Text::from(lines)).style(Style::default().fg(TEXT_MAIN));
    frame.render_widget(paragraph, area);
}

fn render_enemy_sprite(frame: &mut Frame, area: Rect, state: &AppState) {
    if let Some(sprite) = state.enemy_sprite.sprite.as_ref() {
        let (cols, rows) = sprite_fit(sprite, area.width, area.height.saturating_sub(1));
        let sprite_frame = sprite.frame(state.enemy_sprite.frame_index);
        if let Ok(sequence) = sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_ENEMY_BATTLE) {
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
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Sprite on left (wide), stats on right (narrow)
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(22)])
        .split(inner);
    render_player_sprite(frame, layout[0], state);
    render_player_stats(frame, layout[1], state);
}

fn render_player_stats(frame: &mut Frame, area: Rect, state: &AppState) {
    let (current, max) = state
        .battle
        .as_ref()
        .map(|battle| (battle.player_hp, battle.player_hp_max))
        .unwrap_or((state.player_max_hp(), state.player_max_hp()));
    let lines = vec![
        hp_line(current, max),
        Line::from(Span::styled(
            format!("Lv {}", 5 + (max / 10)),
            Style::default().fg(TEXT_DIM),
        )),
    ];
    let paragraph = Paragraph::new(Text::from(lines)).style(Style::default().fg(TEXT_MAIN));
    frame.render_widget(paragraph, area);
}

fn render_player_sprite(frame: &mut Frame, area: Rect, state: &AppState) {
    if let Some(sprite) = state.player_sprite.sprite.as_ref() {
        let (cols, rows) = sprite_fit(sprite, area.width, area.height.saturating_sub(1));
        let sprite_frame = sprite.frame(state.player_sprite.frame_index);
        if let Ok(sequence) = sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_PLAYER_BATTLE) {
            // Center horizontally, align to top
            let offset_x = area.x.saturating_add(area.width.saturating_sub(cols) / 2);
            let offset_y = area.y;
            sprite_backend::set_sprite(SPRITE_ID_PLAYER_BATTLE, offset_x, offset_y, sequence);
            return;
        }
    }

    let content = if state.player_sprite.loading {
        "[loading]"
    } else {
        "[no sprite]"
    };
    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(TEXT_DIM))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_battle_text(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("COMMAND")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(battle) = state.battle.as_ref() else {
        return;
    };

    let mut lines = vec![Line::from(battle.message.clone())];
    match battle.stage {
        BattleStage::Menu => {
            lines.push(Line::from(" "));
            lines.push(battle_menu_line(battle.menu_index));
            lines.push(Line::from(Span::styled(
                "Z/Enter: Select",
                Style::default().fg(TEXT_DIM),
            )));
        }
        BattleStage::Intro | BattleStage::EnemyTurn | BattleStage::Victory | BattleStage::Escape | BattleStage::Defeat => {
            lines.push(Line::from(Span::styled(
                "Z/Enter: Continue",
                Style::default().fg(TEXT_DIM),
            )));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(TEXT_MAIN))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn battle_menu_line(selected: usize) -> Line<'static> {
    let options = ["FIGHT", "RUN"];
    let mut spans = Vec::new();
    for (idx, label) in options.iter().enumerate() {
        let prefix = if idx == selected { ">" } else { " " };
        let style = if idx == selected {
            Style::default().fg(ACCENT_GREEN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT_MAIN)
        };
        spans.push(Span::styled(format!("{} {}", prefix, label), style));
        if idx == 0 {
            spans.push(Span::raw("   "));
        }
    }
    Line::from(spans)
}

fn map_viewport(state: &AppState, view_cols: u16, view_rows: u16) -> (u16, u16) {
    let half_cols = view_cols / 2;
    let half_rows = view_rows / 2;
    let max_x = state.map.width.saturating_sub(view_cols);
    let max_y = state.map.height.saturating_sub(view_rows);
    let mut start_x = state.player.x.saturating_sub(half_cols);
    let mut start_y = state.player.y.saturating_sub(half_rows);
    if start_x > max_x {
        start_x = max_x;
    }
    if start_y > max_y {
        start_y = max_y;
    }
    (start_x, start_y)
}

fn hp_line(current: u16, max: u16) -> Line<'static> {
    let width: usize = 12;
    let ratio = if max == 0 { 0.0 } else { current as f32 / max as f32 };
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
