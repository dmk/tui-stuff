use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use tui_dispatch::{Component, EventKind, EventOutcome, RenderContext};
use tui_dispatch_components::{SelectList, SelectListBehavior, SelectListProps, SelectListStyle, TextInput, TextInputProps, TextInputStyle, Line as CLine};

use crate::action::Action;
use crate::rules::{Ability, BACKGROUND_OPTIONS, CLASS_OPTIONS};
use crate::state::{AppState, CreationStep, Direction as MoveDir, GameMode, LogSpeaker, Tile};

const BG_BASE: Color = Color::Rgb(16, 18, 20);
const PANEL_BG: Color = Color::Rgb(26, 28, 32);
const TEXT_MAIN: Color = Color::Rgb(232, 232, 232);
const TEXT_DIM: Color = Color::Rgb(160, 160, 160);
const ACCENT: Color = Color::Rgb(126, 200, 180);
const ACCENT_GOLD: Color = Color::Rgb(222, 196, 120);
const ACCENT_RED: Color = Color::Rgb(204, 90, 90);

const CELL_ASPECT: f32 = 2.0;
const MAP_TILES_V: u16 = 10;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
    frame.render_widget(Block::default().style(Style::default().bg(BG_BASE)), area);
    if state.mode == GameMode::CharacterCreation {
        render_character_creation(frame, area, state);
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(7), Constraint::Length(3)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(vertical[0]);

    render_map(frame, top[0], state);
    render_sidebar(frame, top[1], state);
    render_log(frame, vertical[1], state);
    render_input(frame, vertical[2], state);
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
    if state.mode == GameMode::CharacterCreation {
        return handle_creation_key(key, state);
    }

    match state.mode {
        GameMode::Dialogue => handle_text_input(key, &state.dialogue.input, Action::DialogueInputChanged, Action::DialogueSubmit),
        GameMode::CustomAction => handle_text_input(key, &state.custom_action.input, Action::CustomActionInputChanged, Action::CustomActionSubmit),
        GameMode::Inventory => match key.code {
            KeyCode::Esc => EventOutcome::action(Action::CloseOverlay),
            _ => EventOutcome::ignored(),
        },
        GameMode::Combat => handle_combat_key(key),
        GameMode::Exploration => handle_exploration_key(key),
        _ => EventOutcome::ignored(),
    }
}

fn handle_exploration_key(key: KeyEvent) -> EventOutcome<Action> {
    match key.code {
        KeyCode::Up | KeyCode::Char('w') => EventOutcome::action(Action::Move(MoveDir::Up)),
        KeyCode::Down | KeyCode::Char('s') => EventOutcome::action(Action::Move(MoveDir::Down)),
        KeyCode::Left | KeyCode::Char('a') => EventOutcome::action(Action::Move(MoveDir::Left)),
        KeyCode::Right | KeyCode::Char('d') => EventOutcome::action(Action::Move(MoveDir::Right)),
        KeyCode::Char('e') => EventOutcome::action(Action::Interact),
        KeyCode::Char('t') => EventOutcome::action(Action::Talk),
        KeyCode::Char('i') => EventOutcome::action(Action::OpenInventory),
        KeyCode::Char('c') => EventOutcome::action(Action::OpenCustomAction),
        KeyCode::PageUp => EventOutcome::action(Action::ScrollLog(2)),
        KeyCode::PageDown => EventOutcome::action(Action::ScrollLog(-2)),
        KeyCode::Esc => EventOutcome::action(Action::Quit),
        _ => EventOutcome::ignored(),
    }
}

fn handle_combat_key(key: KeyEvent) -> EventOutcome<Action> {
    match key.code {
        KeyCode::Up | KeyCode::Char('w') => EventOutcome::action(Action::Move(MoveDir::Up)),
        KeyCode::Down | KeyCode::Char('s') => EventOutcome::action(Action::Move(MoveDir::Down)),
        KeyCode::Left | KeyCode::Char('a') => EventOutcome::action(Action::Move(MoveDir::Left)),
        KeyCode::Right | KeyCode::Char('d') => EventOutcome::action(Action::Move(MoveDir::Right)),
        KeyCode::Char('f') | KeyCode::Enter => EventOutcome::action(Action::CombatAttack),
        KeyCode::Char('e') => EventOutcome::action(Action::CombatEndTurn),
        KeyCode::PageUp => EventOutcome::action(Action::ScrollLog(2)),
        KeyCode::PageDown => EventOutcome::action(Action::ScrollLog(-2)),
        _ => EventOutcome::ignored(),
    }
}

fn handle_creation_key(key: KeyEvent, state: &AppState) -> EventOutcome<Action> {
    match state.creation.step {
        CreationStep::Name => match key.code {
            KeyCode::Enter => EventOutcome::action(Action::CreationNext),
            KeyCode::Backspace => {
                let mut next = state.creation.name.clone();
                next.pop();
                EventOutcome::action(Action::CreationNameChanged(next))
            }
            KeyCode::Char(c) => {
                let mut next = state.creation.name.clone();
                next.push(c);
                EventOutcome::action(Action::CreationNameChanged(next))
            }
            KeyCode::Esc => EventOutcome::action(Action::Quit),
            _ => EventOutcome::ignored(),
        },
        CreationStep::Class => match key.code {
            KeyCode::Up => EventOutcome::action(Action::CreationSelectClass(state.creation.class_index.saturating_sub(1))),
            KeyCode::Down => EventOutcome::action(Action::CreationSelectClass(state.creation.class_index.saturating_add(1))),
            KeyCode::Enter => EventOutcome::action(Action::CreationNext),
            KeyCode::Backspace => EventOutcome::action(Action::CreationBack),
            _ => EventOutcome::ignored(),
        },
        CreationStep::Background => match key.code {
            KeyCode::Up => EventOutcome::action(Action::CreationSelectBackground(state.creation.background_index.saturating_sub(1))),
            KeyCode::Down => EventOutcome::action(Action::CreationSelectBackground(state.creation.background_index.saturating_add(1))),
            KeyCode::Enter => EventOutcome::action(Action::CreationNext),
            KeyCode::Backspace => EventOutcome::action(Action::CreationBack),
            _ => EventOutcome::ignored(),
        },
        CreationStep::Stats => match key.code {
            KeyCode::Up => EventOutcome::action(Action::CreationSelectStat(state.creation.selected_stat.saturating_sub(1))),
            KeyCode::Down => EventOutcome::action(Action::CreationSelectStat(state.creation.selected_stat.saturating_add(1))),
            KeyCode::Left => EventOutcome::action(Action::CreationAdjustStat(-1)),
            KeyCode::Right => EventOutcome::action(Action::CreationAdjustStat(1)),
            KeyCode::Enter => EventOutcome::action(Action::CreationNext),
            KeyCode::Backspace => EventOutcome::action(Action::CreationBack),
            _ => EventOutcome::ignored(),
        },
        CreationStep::Confirm => match key.code {
            KeyCode::Enter => EventOutcome::action(Action::CreationConfirm),
            KeyCode::Backspace => EventOutcome::action(Action::CreationBack),
            _ => EventOutcome::ignored(),
        },
    }
}

fn handle_text_input(
    key: KeyEvent,
    current: &str,
    on_change: fn(String) -> Action,
    on_submit: Action,
) -> EventOutcome<Action> {
    match key.code {
        KeyCode::Esc => EventOutcome::action(Action::CloseOverlay),
        KeyCode::Enter => EventOutcome::action(on_submit),
        KeyCode::Backspace => {
            let mut next = current.to_string();
            next.pop();
            EventOutcome::action(on_change(next))
        }
        KeyCode::Char(c) => {
            let mut next = current.to_string();
            next.push(c);
            EventOutcome::action(on_change(next))
        }
        _ => EventOutcome::ignored(),
    }
}

fn render_character_creation(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title("Character Creation")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)])
        .split(inner);

    let header = Paragraph::new(Text::from(vec![Line::from(vec![Span::styled(
        format!("Step: {:?}", state.creation.step),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )])]))
    .alignment(Alignment::Left);
    frame.render_widget(header, chunks[0]);

    match state.creation.step {
        CreationStep::Name => render_creation_name(frame, chunks[1], state),
        CreationStep::Class => render_creation_list(frame, chunks[1], "Class", CLASS_OPTIONS, state.creation.class_index),
        CreationStep::Background => render_creation_list(frame, chunks[1], "Background", BACKGROUND_OPTIONS, state.creation.background_index),
        CreationStep::Stats => render_creation_stats(frame, chunks[1], state),
        CreationStep::Confirm => render_creation_confirm(frame, chunks[1], state),
    }

    let footer = Paragraph::new(Text::from(vec![Line::from(vec![Span::styled(
        "Enter=Next  Backspace=Back",
        Style::default().fg(TEXT_DIM),
    )])]))
    .alignment(Alignment::Left);
    frame.render_widget(footer, chunks[2]);
}

fn render_creation_name(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut input = TextInput::new();
    let props = TextInputProps {
        value: &state.creation.name,
        placeholder: "Name",
        is_focused: true,
        style: TextInputStyle::default(),
        on_change: Action::CreationNameChanged,
        on_submit: |_| Action::CreationNext,
        on_cursor_move: None,
    };
    input.render(frame, area, props);
}

fn render_creation_list(frame: &mut Frame, area: Rect, title: &str, items: &[&str], selected: usize) {
    let block = Block::default().title(title).borders(Borders::ALL).style(Style::default().bg(PANEL_BG));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let render_item = |item: &CLine<'static>| item.clone();
    let lines: Vec<CLine<'static>> = items
        .iter()
        .map(|item| CLine::from(item.to_string()))
        .collect();
    let mut list = SelectList::new();
    let props = SelectListProps {
        items: &lines,
        count: lines.len(),
        selected,
        is_focused: true,
        style: SelectListStyle::default(),
        behavior: SelectListBehavior::default(),
        on_select: noop_select,
        render_item: &render_item,
    };
    list.render(frame, inner, props);
}

fn render_creation_stats(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().title("Stats (Point Buy)").borders(Borders::ALL).style(Style::default().bg(PANEL_BG));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let stats = [
        (Ability::Strength, "Strength", state.creation.stats.strength),
        (Ability::Dexterity, "Dexterity", state.creation.stats.dexterity),
        (Ability::Constitution, "Constitution", state.creation.stats.constitution),
        (Ability::Intelligence, "Intelligence", state.creation.stats.intelligence),
        (Ability::Wisdom, "Wisdom", state.creation.stats.wisdom),
        (Ability::Charisma, "Charisma", state.creation.stats.charisma),
    ];
    let mut lines = Vec::new();
    for (idx, (_, label, score)) in stats.iter().enumerate() {
        let mut style = Style::default().fg(TEXT_MAIN);
        if idx == state.creation.selected_stat {
            style = style.fg(ACCENT).add_modifier(Modifier::BOLD);
        }
        lines.push(Line::from(vec![
            Span::styled(format!("{label:<12}"), style),
            Span::styled(format!("{score:>2}"), style),
        ]));
    }
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        format!("Points remaining: {}", state.creation.points_remaining),
        Style::default().fg(ACCENT_GOLD),
    )));
    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_creation_confirm(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled("Review", Style::default().fg(ACCENT))));
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(format!("Name: {}", state.creation.name)));
    let class_name = CLASS_OPTIONS.get(state.creation.class_index).copied().unwrap_or("Adventurer");
    lines.push(Line::from(format!("Class: {class_name}")));
    let background = BACKGROUND_OPTIONS
        .get(state.creation.background_index)
        .copied()
        .unwrap_or("Wanderer");
    lines.push(Line::from(format!("Background: {background}")));
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        "Press Enter to begin.",
        Style::default().fg(ACCENT_GOLD),
    )));
    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_map(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(state.map.name.as_str())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL_BG));
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

    for item in &state.items {
        draw_marker(buf, item.x, item.y, start_x, start_y, view_tiles_h, view_tiles_v, origin_x, origin_y, cols_per_tile, rows_per_tile, '*', ACCENT_GOLD);
    }
    for npc in &state.npcs {
        draw_marker(buf, npc.x, npc.y, start_x, start_y, view_tiles_h, view_tiles_v, origin_x, origin_y, cols_per_tile, rows_per_tile, 'N', ACCENT);
    }
    for encounter in &state.encounters {
        if encounter.defeated {
            continue;
        }
        draw_marker(buf, encounter.x, encounter.y, start_x, start_y, view_tiles_h, view_tiles_v, origin_x, origin_y, cols_per_tile, rows_per_tile, 'E', ACCENT_RED);
    }
    let (px, py) = state.player_pos();
    draw_marker(buf, px, py, start_x, start_y, view_tiles_h, view_tiles_v, origin_x, origin_y, cols_per_tile, rows_per_tile, '@', TEXT_MAIN);
}

fn draw_marker(
    buf: &mut ratatui::buffer::Buffer,
    map_x: u16,
    map_y: u16,
    start_x: u16,
    start_y: u16,
    view_tiles_h: u16,
    view_tiles_v: u16,
    origin_x: u16,
    origin_y: u16,
    cols_per_tile: u16,
    rows_per_tile: u16,
    ch: char,
    fg: Color,
) {
    if map_x < start_x || map_y < start_y || map_x >= start_x + view_tiles_h || map_y >= start_y + view_tiles_v {
        return;
    }
    let tile_col = map_x - start_x;
    let tile_row = map_y - start_y;
    let cell_x = origin_x + tile_col * cols_per_tile;
    let cell_y = origin_y + tile_row * rows_per_tile;
    let center_x = cell_x + cols_per_tile / 2;
    let center_y = cell_y + rows_per_tile / 2;
    if let Some(cell) = buf.cell_mut((center_x, center_y)) {
        cell.set_fg(fg).set_char(ch);
    }
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title("Status")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled("Name: ", Style::default().fg(TEXT_DIM)), Span::raw(&state.player.name)]));
    lines.push(Line::from(vec![Span::styled("Class: ", Style::default().fg(TEXT_DIM)), Span::raw(&state.player.class_name)]));
    lines.push(Line::from(vec![Span::styled("BG: ", Style::default().fg(TEXT_DIM)), Span::raw(&state.player.background)]));
    lines.push(Line::from(vec![Span::styled("HP: ", Style::default().fg(TEXT_DIM)), Span::raw(format!("{}/{}", state.player.hp, state.player.max_hp))]));
    lines.push(Line::from(vec![Span::styled("Pos: ", Style::default().fg(TEXT_DIM)), Span::raw(format!("{},{}", state.player.x, state.player.y))]));
    lines.push(Line::from(Span::raw("")));

    if let Some(combat) = &state.combat {
        if let Some(enemy) = state.encounters.iter().find(|e| e.id == combat.enemy_id) {
            lines.push(Line::from(Span::styled("Combat", Style::default().fg(ACCENT_RED))));
            lines.push(Line::from(format!("Enemy: {}", enemy.name)));
            lines.push(Line::from(format!("HP: {}", enemy.hp.max(0))));
            lines.push(Line::from(format!("Move left: {}", combat.movement_left)));
            lines.push(Line::from(Span::raw("")));
        }
    }

    if let Some(pending) = &state.pending_llm {
        let label = match pending {
            crate::state::PendingLlm::Dialogue { .. } => "Talking",
            crate::state::PendingLlm::CustomAction => "Interpreting",
        };
        let spinner = spinner_frame(state.spinner_frame);
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            format!("DM {spinner} {label}...",),
            Style::default().fg(ACCENT),
        )));
    }

    lines.push(Line::from(Span::styled("Inventory", Style::default().fg(ACCENT_GOLD))));
    if state.player.inventory.is_empty() {
        lines.push(Line::from(Span::styled("(empty)", Style::default().fg(TEXT_DIM))));
    } else {
        for item in &state.player.inventory {
            lines.push(Line::from(format!("- {} x{}", item.name, item.qty)));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_log(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title("Log")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    let width = inner.width as usize;
    for entry in &state.log {
        let (label, color) = match entry.speaker {
            LogSpeaker::System => ("[System]", TEXT_DIM),
            LogSpeaker::Player => ("[You]", ACCENT),
            LogSpeaker::Npc => ("[NPC]", TEXT_MAIN),
            LogSpeaker::Combat => ("[Combat]", ACCENT_RED),
        };
        let prefix_width = label.chars().count() + 1;
        let wrap_width = width.saturating_sub(prefix_width).max(1);
        let wrapped = wrap_text(&entry.text, wrap_width);
        for (idx, chunk) in wrapped.into_iter().enumerate() {
            if idx == 0 {
                lines.push(Line::from(vec![
                    Span::styled(label, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::raw(chunk),
                ]));
            } else {
                let indent = " ".repeat(prefix_width);
                lines.push(Line::from(vec![
                    Span::styled(indent, Style::default().fg(TEXT_DIM)),
                    Span::raw(chunk),
                ]));
            }
        }
    }

    let view_height = inner.height as usize;
    let total_lines = lines.len();
    let max_start = total_lines.saturating_sub(view_height);
    let offset = state.log_scroll as usize;
    let start = max_start.saturating_sub(offset);
    let visible = if total_lines > start { lines[start..].to_vec() } else { Vec::new() };

    let paragraph = Paragraph::new(Text::from(visible)).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_input(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title("Input")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = match state.mode {
        GameMode::Dialogue => format!("Say: {}", state.dialogue.input),
        GameMode::CustomAction => format!("Action: {}", state.custom_action.input),
        GameMode::Combat => "F=Attack  E=End Turn  Move=Arrows".to_string(),
        GameMode::Inventory => "Esc to close inventory".to_string(),
        _ => "WASD/Arrows move | E interact | T talk | C custom action | I inventory".to_string(),
    };
    let paragraph = Paragraph::new(Text::from(Line::from(text))).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn tile_colors(tile: Tile) -> (Color, Color) {
    match tile {
        Tile::Grass => (Color::Rgb(34, 112, 58), Color::Rgb(38, 120, 64)),
        Tile::Road => (Color::Rgb(156, 132, 76), Color::Rgb(150, 128, 74)),
        Tile::Floor => (Color::Rgb(90, 90, 94), Color::Rgb(80, 80, 84)),
        Tile::Wall => (Color::Rgb(66, 74, 66), Color::Rgb(60, 68, 60)),
        Tile::Water => (Color::Rgb(48, 86, 146), Color::Rgb(52, 92, 150)),
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

fn spinner_frame(frame: u8) -> char {
    match frame % 4 {
        0 => '|',
        1 => '/',
        2 => '-',
        _ => '\\',
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for raw in text.lines() {
        if raw.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in raw.split_whitespace() {
            let word_len = word.chars().count();
            let line_len = line.chars().count();
            if line.is_empty() {
                if word_len <= width {
                    line.push_str(word);
                } else {
                    out.extend(chunk_word(word, width));
                }
            } else if line_len + 1 + word_len <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(line);
                line = String::new();
                if word_len <= width {
                    line.push_str(word);
                } else {
                    out.extend(chunk_word(word, width));
                }
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn chunk_word(word: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        out.push(String::new());
        return out;
    }
    for chunk in chars.chunks(width.max(1)) {
        out.push(chunk.iter().collect::<String>());
    }
    out
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
            0 => ('.', adjust_color(Color::Rgb(34, 112, 58), 10), 6),
            1 => ('\'', adjust_color(Color::Rgb(34, 112, 58), 6), 7),
            _ => ('`', adjust_color(Color::Rgb(34, 112, 58), -4), 8),
        },
        Tile::Road => match variant {
            0 => ('.', adjust_color(Color::Rgb(156, 132, 76), 6), 6),
            1 => (':', adjust_color(Color::Rgb(156, 132, 76), 4), 7),
            _ => ('\'', adjust_color(Color::Rgb(156, 132, 76), -4), 8),
        },
        Tile::Floor => match variant {
            0 => ('.', adjust_color(Color::Rgb(90, 90, 94), 6), 6),
            1 => (':', adjust_color(Color::Rgb(90, 90, 94), 4), 7),
            _ => ('\'', adjust_color(Color::Rgb(90, 90, 94), -4), 8),
        },
        Tile::Wall => match variant {
            0 => ('#', adjust_color(Color::Rgb(66, 74, 66), 10), 8),
            1 => ('+', adjust_color(Color::Rgb(66, 74, 66), 6), 9),
            _ => ('.', adjust_color(Color::Rgb(66, 74, 66), 4), 10),
        },
        Tile::Water => match variant {
            0 => ('~', adjust_color(Color::Rgb(48, 86, 146), 10), 8),
            1 => ('-', adjust_color(Color::Rgb(48, 86, 146), 6), 9),
            _ => ('.', adjust_color(Color::Rgb(48, 86, 146), 4), 10),
        },
    }
}

fn map_viewport(state: &AppState, view_cols: u16, view_rows: u16) -> (u16, u16) {
    let (x, y) = state.player_pos();
    let half_w = view_cols / 2;
    let half_h = view_rows / 2;
    let max_x = state.map.width.saturating_sub(view_cols);
    let max_y = state.map.height.saturating_sub(view_rows);
    let start_x = x.saturating_sub(half_w).min(max_x);
    let start_y = y.saturating_sub(half_h).min(max_y);
    (start_x, start_y)
}

fn noop_select(_: usize) -> Action {
    Action::Tick
}
