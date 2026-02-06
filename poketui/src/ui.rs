use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{block::Title, Block, BorderType, Borders, Paragraph, Wrap},
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
const SPRITE_ID_PLAYER_BATTLE: u32 = 4;

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

fn tile_colors(tile: Tile) -> (Color, Color) {
    match tile {
        Tile::Grass => (TILE_GRASS, TILE_GRASS_ALT),
        Tile::Path => (TILE_PATH, TILE_PATH_ALT),
        Tile::Sand => (TILE_SAND, TILE_SAND_ALT),
        Tile::Wall => (TILE_WALL, TILE_WALL_ALT),
        Tile::Water => (TILE_WATER, TILE_WATER_ALT),
    }
}

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

fn tile_seed(x: u16, y: u16) -> u32 {
    let mut n = x as u32;
    n = n
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263));
    n ^= n >> 13;
    n = n.wrapping_mul(1274126177);
    n ^= n >> 16;
    n
}

fn cell_seed(x: u16, y: u16, dx: u16, dy: u16) -> u32 {
    let mut n = tile_seed(x, y);
    n ^= (dx as u32).wrapping_mul(2246822519);
    n ^= (dy as u32).wrapping_mul(3266489917);
    n ^= n >> 15;
    n = n.wrapping_mul(668265263);
    n ^= n >> 13;
    n
}

fn tile_texture(tile: Tile, map_x: u16, map_y: u16) -> (char, Color, u8) {
    let seed = tile_seed(map_x, map_y);
    let variant = (seed % 3) as u8;
    match tile {
        Tile::Grass => match variant {
            0 => ('.', adjust_color(TILE_GRASS, 10), 6),
            1 => ('\'', adjust_color(TILE_GRASS, 6), 7),
            _ => ('`', adjust_color(TILE_GRASS, -4), 8),
        },
        Tile::Sand => match variant {
            0 => (':', adjust_color(TILE_SAND, 8), 6),
            1 => ('.', adjust_color(TILE_SAND, 4), 7),
            _ => (',', adjust_color(TILE_SAND, -4), 8),
        },
        Tile::Path => match variant {
            0 => ('.', adjust_color(TILE_PATH, 6), 6),
            1 => (':', adjust_color(TILE_PATH, 4), 7),
            _ => ('\'', adjust_color(TILE_PATH, -4), 8),
        },
        Tile::Wall => match variant {
            0 => ('#', adjust_color(TILE_WALL, 10), 8),
            1 => ('+', adjust_color(TILE_WALL, 6), 9),
            _ => ('.', adjust_color(TILE_WALL, 4), 10),
        },
        Tile::Water => match variant {
            0 => ('~', adjust_color(TILE_WATER, 10), 8),
            1 => ('-', adjust_color(TILE_WATER, 6), 9),
            _ => ('.', adjust_color(TILE_WATER, 4), 10),
        },
    }
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

fn render_pause_menu(frame: &mut Frame, area: Rect, state: &AppState) {
    // Clear sprites so they don't show through the modal
    sprite_backend::clear_sprites();

    // Dim the background
    let buf = frame.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &mut buf[(x, y)];
            // Darken existing content
            if let Color::Rgb(r, g, b) = cell.bg {
                cell.bg = Color::Rgb(r / 2, g / 2, b / 2);
            }
            if let Color::Rgb(r, g, b) = cell.fg {
                cell.fg = Color::Rgb(r / 2, g / 2, b / 2);
            }
        }
    }

    // Draw modal in center
    let modal_width = 24;
    let modal_height = 10;
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear the modal area first (fill with spaces and background color)
    let buf = frame.buffer_mut();
    for y in modal_area.y..modal_area.y + modal_area.height {
        for x in modal_area.x..modal_area.x + modal_area.width {
            buf[(x, y)].set_char(' ').set_bg(BG_PANEL).set_fg(TEXT_MAIN);
        }
    }

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
    let line = Line::from(vec![
        Span::styled(
            title,
            Style::default()
                .fg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  •  "),
        Span::styled(
            format!("Partner {} Lv {}", player, state.player_level),
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

    let hp_max = state.player_max_hp();
    let hp_current = state.player_hp.min(hp_max);
    let exp_current = state.exp_progress();
    let exp_next = state.exp_to_next_level().max(1);
    let lines = vec![
        Line::from(Span::styled(
            format_name(&state.player_name()),
            Style::default()
                .fg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Lv {}", state.player_level)),
        Line::from(""),
        meter_line("HP", hp_current as u32, hp_max as u32, 12, ACCENT_GREEN),
        meter_line("EXP", exp_current, exp_next, 12, ACCENT_GOLD),
        Line::from(""),
        Line::from(Span::styled(
            format!("Bag: {}", inventory_summary(state)),
            Style::default().fg(TEXT_MAIN),
        )),
    ];
    let paragraph = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
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

    // Each tile spans multiple terminal cells for a zoomed-in view
    let rows_per_tile = (inner.height / MAP_TILES_V).max(2);
    let cols_per_tile = ((rows_per_tile as f32 * CELL_ASPECT).round() as u16).max(2);

    let view_tiles_h = (inner.width / cols_per_tile).min(state.map.width);
    let view_tiles_v = (inner.height / rows_per_tile).min(state.map.height);
    if view_tiles_h == 0 || view_tiles_v == 0 {
        return;
    }

    let used_cols = view_tiles_h * cols_per_tile;
    let used_rows = view_tiles_v * rows_per_tile;
    let pad_x = (inner.width.saturating_sub(used_cols)) / 2;
    let pad_y = (inner.height.saturating_sub(used_rows)) / 2;
    let origin_x = inner.x + pad_x;
    let origin_y = inner.y + pad_y;

    let (start_x, start_y) = map_viewport(state, view_tiles_h, view_tiles_v);
    let buf = frame.buffer_mut();

    // Draw tiles
    for tile_row in 0..view_tiles_v {
        for tile_col in 0..view_tiles_h {
            let map_x = start_x + tile_col;
            let map_y = start_y + tile_row;
            let tile = state.map.tile(map_x, map_y);
            let (color_main, color_alt) = tile_colors(tile);
            let seed = tile_seed(map_x, map_y);
            let bg = if seed % 2 == 0 { color_main } else { color_alt };
            let (tex_char, tex_color, density) = tile_texture(tile, map_x, map_y);

            let cell_x = origin_x + tile_col * cols_per_tile;
            let cell_y = origin_y + tile_row * rows_per_tile;

            // Draw the tile interior with subtle pattern
            for dy in 0..rows_per_tile {
                for dx in 0..cols_per_tile {
                    let x = cell_x + dx;
                    let y = cell_y + dy;
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        let sprinkle = cell_seed(map_x, map_y, dx, dy);
                        if sprinkle % density as u32 == 0 {
                            cell.set_bg(bg).set_fg(tex_color).set_char(tex_char);
                        } else {
                            cell.set_bg(bg).set_fg(bg).set_char(' ');
                        }
                    }
                }
            }
        }
    }

    // Draw player sprite on top
    let player_sprite = match state.player.facing {
        MoveDir::Right => state
            .player_sprite
            .sprite_flipped
            .as_ref()
            .or(state.player_sprite.sprite.as_ref()),
        _ => state.player_sprite.sprite.as_ref(),
    };
    if let Some(sprite) = player_sprite {
        let player_col = state.player.x.saturating_sub(start_x);
        let player_row = state.player.y.saturating_sub(start_y);
        if player_col < view_tiles_h && player_row < view_tiles_v {
            let (cols, rows) = sprite_fit(sprite, cols_per_tile, rows_per_tile);
            let sprite_frame = sprite.frame(state.player_sprite.frame_index);
            if let Ok(sequence) =
                sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_PLAYER_MAP)
            {
                let tile_x = origin_x + player_col * cols_per_tile;
                let tile_y = origin_y + player_row * rows_per_tile;
                let offset_x = tile_x + cols_per_tile.saturating_sub(cols) / 2;
                let offset_y = tile_y + rows_per_tile.saturating_sub(rows) / 2;
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
    let lines = if show_hud {
        vec![
            Line::from(Span::styled(message, Style::default().fg(TEXT_MAIN))),
            Line::from(Span::styled(
                "Arrows/WASD move  |  Esc menu",
                Style::default().fg(TEXT_DIM),
            )),
        ]
    } else {
        let player = format_name(&state.player_name());
        let hp_max = state.player_max_hp();
        let hp_current = state.player_hp.min(hp_max);
        let exp_current = state.exp_progress();
        let exp_next = state.exp_to_next_level().max(1);
        let bag_summary = inventory_summary(state);
        let bar_width = if area.width >= 80 { 26 } else { 16 };
        vec![
            Line::from(format!("Partner {}  Lv {}", player, state.player_level)),
            meter_line(
                "HP",
                hp_current as u32,
                hp_max as u32,
                bar_width,
                ACCENT_GREEN,
            ),
            meter_line("EXP", exp_current, exp_next, bar_width, ACCENT_GOLD),
            Line::from(format!(
                "Steps {}  |  Bag {}",
                state.player.steps, bag_summary
            )),
            Line::from(Span::styled(message, Style::default().fg(TEXT_MAIN))),
            Line::from(Span::styled(
                "Arrows/WASD move  |  Esc menu",
                Style::default().fg(TEXT_DIM),
            )),
        ]
    };
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
    let block = panel_block(title.as_str(), BG_PANEL);
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
            format!("Lv {}", battle.enemy_level),
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
            format!("Lv {}", state.player_level),
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
        if let Ok(sequence) =
            sprite::kitty_sequence(sprite_frame, cols, rows, SPRITE_ID_PLAYER_BATTLE)
        {
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
    let block = panel_block("COMMAND", BG_PANEL_ALT);
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
        BattleStage::ItemMenu => {
            lines.push(Line::from(" "));
            let item_lines = battle_item_lines(state, battle.item_index);
            lines.extend(item_lines);
            lines.push(Line::from(Span::styled(
                "Z/Enter: Use  |  Esc: Back",
                Style::default().fg(TEXT_DIM),
            )));
        }
        BattleStage::Intro
        | BattleStage::EnemyTurn
        | BattleStage::Victory
        | BattleStage::Escape
        | BattleStage::Defeat => {
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
    let options = ["FIGHT", "BAG", "RUN"];
    let mut spans = Vec::new();
    for (idx, label) in options.iter().enumerate() {
        let style = if idx == selected {
            Style::default()
                .fg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT_MAIN)
        };
        spans.push(Span::styled(label.to_string(), style));
        if idx + 1 < options.len() {
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

fn inventory_summary(state: &AppState) -> String {
    let mut items = Vec::new();
    for stack in &state.inventory {
        if stack.qty > 0 {
            items.push(format!("{} x{}", stack.kind.label(), stack.qty));
        }
    }
    if items.is_empty() {
        "empty".to_string()
    } else {
        items.join(", ")
    }
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
