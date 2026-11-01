use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Sparkline},
    Frame,
};
use tui_dispatch::{EventKind, EventOutcome};

use crate::{action::Action, patterns::Pattern, state::*};

pub const BACKGROUND: Color = Color::Rgb(8, 12, 20);
pub const FIELD: Color = Color::Rgb(10, 17, 27);
pub const MINT: Color = Color::Rgb(160, 248, 194);
const CYAN: Color = Color::Rgb(101, 202, 218);
const TEXT: Color = Color::Rgb(203, 217, 227);
const MUTED: Color = Color::Rgb(112, 136, 155);
const LINE: Color = Color::Rgb(36, 56, 73);
const GOLD: Color = Color::Rgb(236, 196, 124);

#[derive(Clone, Copy, Debug)]
pub struct ScreenLayout {
    pub field: Rect,
    pub sidebar: Option<Rect>,
}

impl ScreenLayout {
    pub fn new(area: Rect) -> Self {
        let width = area.width.saturating_sub(4);
        let height = area.height.saturating_sub(9);
        let sidebar_width = if area.width >= 105 { 29 } else { 0 };
        Self {
            field: Rect::new(
                area.x + 2,
                area.y + 5,
                width.saturating_sub(sidebar_width),
                height,
            ),
            sidebar: (sidebar_width > 0)
                .then(|| Rect::new(area.x + area.width - 29, area.y + 5, 27, height)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BoardView {
    pub area: Rect,
    pub origin_x: i32,
    pub origin_y: i32,
    pub cols: i32,
    pub rows: i32,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl BoardView {
    pub fn new(state: &AppState, field: Rect) -> Self {
        let (cell_width, cell_height) = [(1, 1), (2, 1), (4, 2)][state.zoom];
        let area = field.inner(ratatui::layout::Margin::new(1, 1));
        let cols = i32::from(area.width / cell_width);
        let rows = i32::from(area.height / cell_height);
        Self {
            area,
            cols,
            rows,
            cell_width,
            cell_height,
            origin_x: (state.camera_x - cols / 2).clamp(0, (WORLD_WIDTH - cols).max(0)),
            origin_y: (state.camera_y - rows / 2).clamp(0, (WORLD_HEIGHT - rows).max(0)),
        }
    }

    pub fn for_state(state: &AppState) -> Self {
        Self::new(
            state,
            ScreenLayout::new(Rect::new(0, 0, state.viewport_width, state.viewport_height)).field,
        )
    }

    pub fn world_at(self, column: u16, row: u16) -> Option<(i32, i32)> {
        if !self.area.contains((column, row).into()) {
            return None;
        }
        let x = i32::from((column - self.area.x) / self.cell_width);
        let y = i32::from((row - self.area.y) / self.cell_height);
        if x >= self.cols || y >= self.rows {
            return None;
        }
        let world = (self.origin_x + x, self.origin_y + y);
        AppState::index(world.0, world.1).map(|_| world)
    }
}

pub fn map_event(event: &EventKind, state: &AppState) -> EventOutcome<Action> {
    let action = match event {
        EventKind::Resize(width, height) => Some(Action::ViewportResize {
            width: *width,
            height: *height,
        }),
        EventKind::Key(key) if key.kind != KeyEventKind::Release => {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                Some(Action::Quit)
            } else if state.show_help {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('?') => Some(Action::HelpToggle),
                    KeyCode::Char('q') => Some(Action::Quit),
                    _ => None,
                }
            } else {
                match key.code {
                    KeyCode::Char('q') => Some(Action::Quit),
                    KeyCode::Char(' ') => Some(Action::PauseToggle),
                    KeyCode::Char('n') => Some(Action::Step),
                    KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::SpeedChange(1)),
                    KeyCode::Char('-') => Some(Action::SpeedChange(-1)),
                    KeyCode::Char('1'..='6') => {
                        let KeyCode::Char(key) = key.code else {
                            unreachable!()
                        };
                        Some(Action::PatternSelect(
                            Pattern::ALL[(key as u8 - b'1') as usize],
                        ))
                    }
                    KeyCode::Char('r') => Some(Action::PatternLoad),
                    KeyCode::Enter => Some(Action::PatternStamp),
                    KeyCode::Char('x') => Some(Action::CellToggle),
                    KeyCode::Char('c') => Some(Action::Clear),
                    KeyCode::Char('f') => Some(Action::Randomize),
                    KeyCode::Char('b') => Some(Action::BoundaryToggle),
                    KeyCode::Char('g') => Some(Action::GridToggle),
                    KeyCode::Char('t') => Some(Action::TrailsToggle),
                    KeyCode::Char('z') => Some(Action::ZoomCycle),
                    KeyCode::Char('?') => Some(Action::HelpToggle),
                    KeyCode::Home | KeyCode::Char('0') => Some(Action::CameraHome),
                    KeyCode::Left => Some(Action::CursorMove { dx: -1, dy: 0 }),
                    KeyCode::Right => Some(Action::CursorMove { dx: 1, dy: 0 }),
                    KeyCode::Up => Some(Action::CursorMove { dx: 0, dy: -1 }),
                    KeyCode::Down => Some(Action::CursorMove { dx: 0, dy: 1 }),
                    KeyCode::Char('a') => Some(Action::CameraPan { dx: -4, dy: 0 }),
                    KeyCode::Char('d') => Some(Action::CameraPan { dx: 4, dy: 0 }),
                    KeyCode::Char('w') => Some(Action::CameraPan { dx: 0, dy: -4 }),
                    KeyCode::Char('s') => Some(Action::CameraPan { dx: 0, dy: 4 }),
                    _ => None,
                }
            }
        }
        EventKind::Mouse(mouse) if !state.show_help && state.ready() => {
            let view = BoardView::for_state(state);
            let alive = match mouse.kind {
                MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left) => Some(true),
                MouseEventKind::Down(MouseButton::Right)
                | MouseEventKind::Drag(MouseButton::Right) => Some(false),
                _ => None,
            };
            if let Some((x, y)) = view.world_at(mouse.column, mouse.row) {
                alive.map(|alive| Action::CellSet { x, y, alive })
            } else if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                let layout =
                    ScreenLayout::new(Rect::new(0, 0, state.viewport_width, state.viewport_height));
                layout.sidebar.and_then(|panel| {
                    let index = mouse.row.checked_sub(panel.y + 3)? as usize;
                    (panel.contains((mouse.column, mouse.row).into()) && index < 6)
                        .then(|| Action::PatternSelect(Pattern::ALL[index]))
                })
            } else {
                None
            }
        }
        EventKind::Scroll { delta, .. } if !state.show_help => Some(Action::CameraPan {
            dx: 0,
            dy: delta.signum() as i32 * 3,
        }),
        _ => None,
    };
    match action {
        Some(action) => EventOutcome::action(action).with_render(),
        None => EventOutcome::default(),
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    frame.render_widget(
        Block::default().style(Style::default().bg(BACKGROUND).fg(TEXT)),
        area,
    );
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame.render_widget(
            Paragraph::new(format!(
            "LIFE\n\nResize to at least {MIN_WIDTH} x {MIN_HEIGHT}\nSimulation suspended. Q quits."
        ))
            .alignment(Alignment::Center)
            .style(Style::default().fg(MINT)),
            area,
        );
        return;
    }
    let status = if state.show_help {
        "HELP"
    } else if state.running {
        "EVOLVING"
    } else {
        "PAUSED"
    };
    write(
        frame,
        Rect::new(area.x + 2, area.y + 1, 28, 1),
        Line::from(vec![
            Span::styled(
                "✳  L I F E",
                Style::default().fg(MINT).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  /  observatory", Style::default().fg(MUTED)),
        ]),
    );
    write(
        frame,
        Rect::new(area.x + area.width - 26, area.y + 1, 24, 1),
        Line::from(vec![
            Span::styled(
                format!("● {status:<9}"),
                Style::default().fg(if state.running { MINT } else { GOLD }),
            ),
            Span::styled("  B3 / S23", Style::default().fg(MUTED)),
        ]),
    );
    write(
        frame,
        Rect::new(area.x + 2, area.y + 3, area.width - 4, 1),
        Line::from(vec![
            Span::styled("GEN ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{:06}    ", state.generation),
                Style::default().fg(TEXT),
            ),
            Span::styled("ALIVE ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{:05}    ", state.population),
                Style::default().fg(MINT),
            ),
            Span::styled(
                format!("{} gen/s", state.speed()),
                Style::default().fg(CYAN),
            ),
        ]),
    );

    let layout = ScreenLayout::new(area);
    render_field(frame, state, layout.field);
    if let Some(panel) = layout.sidebar {
        render_sidebar(frame, state, panel);
    }
    let footer_y = area.y + area.height - 3;
    write(
        frame,
        Rect::new(area.x + 2, footer_y, area.width - 4, 1),
        Line::from(vec![
            Span::styled("SPACE", Style::default().fg(MINT)),
            Span::raw(" run/pause  "),
            Span::styled("N", Style::default().fg(MINT)),
            Span::raw(" step  "),
            Span::styled("1–6", Style::default().fg(MINT)),
            Span::raw(" select  "),
            Span::styled("R", Style::default().fg(MINT)),
            Span::raw(" load  "),
            Span::styled("?", Style::default().fg(MINT)),
            Span::raw(" help"),
        ])
        .style(Style::default().fg(MUTED)),
    );
    write(
        frame,
        Rect::new(area.x + 2, footer_y + 1, area.width - 4, 1),
        Line::from(if layout.sidebar.is_some() {
            format!(
                "Paint / erase: mouse   WASD pan   Z zoom   ± speed   {} edges   Q quit",
                if state.wrapping { "wrapping" } else { "fixed" }
            )
        } else {
            format!("↵ stamp {}   Z zoom   Q quit", state.selected.name())
        })
        .style(Style::default().fg(MUTED)),
    );
    if state.show_help {
        render_help(frame, area);
    }
}

fn render_field(frame: &mut Frame, state: &AppState, area: Rect) {
    let view = BoardView::new(state, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(LINE))
        .style(Style::default().bg(FIELD))
        .title(
            Line::from(format!("  {}  ", state.scene.to_uppercase()))
                .style(Style::default().fg(MUTED)),
        )
        .title_bottom(
            Line::from(format!(
                "  {}×{} world · view {},{} · {}×  ",
                WORLD_WIDTH,
                WORLD_HEIGHT,
                view.origin_x,
                view.origin_y,
                [1, 2, 4][state.zoom]
            ))
            .style(Style::default().fg(MUTED)),
        );
    frame.render_widget(block, area);
    for row in 0..view.rows {
        for col in 0..view.cols {
            let (x, y) = (view.origin_x + col, view.origin_y + row);
            let Some(index) = AppState::index(x, y) else {
                continue;
            };
            let age = state.cells[index];
            let trail = state.trails[index];
            let px = view.area.x + col as u16 * view.cell_width;
            let py = view.area.y + row as u16 * view.cell_height;
            for dy in 0..view.cell_height {
                for dx in 0..view.cell_width {
                    let cell = &mut frame.buffer_mut()[(px + dx, py + dy)];
                    if age > 0 {
                        let (bright, base) = age_colors(age);
                        cell.set_char('▀').set_fg(bright).set_bg(base);
                    } else if state.show_trails && trail > 0 {
                        cell.set_char('░')
                            .set_fg(Color::Rgb(23 + trail * 4, 24 + trail * 2, 41 + trail * 5))
                            .set_bg(FIELD);
                    } else {
                        let dot = state.show_grid && x % 4 == 0 && y % 4 == 0 && dx == 0 && dy == 0;
                        cell.set_char(if dot { '·' } else { ' ' })
                            .set_fg(Color::Rgb(35, 52, 66))
                            .set_bg(FIELD);
                    }
                    if state.cursor_visible && (x, y) == (state.cursor_x, state.cursor_y) {
                        cell.set_style(Style::default().add_modifier(Modifier::UNDERLINED));
                        if age == 0 {
                            cell.set_char('▁').set_fg(GOLD);
                        }
                    }
                }
            }
        }
    }
}

pub fn age_colors(age: u16) -> (Color, Color) {
    let (r, g, b): (u8, u8, u8) = match age {
        0..=1 => (186, 255, 205),
        2..=4 => (111, 229, 190),
        5..=12 => (71, 187, 191),
        13..=30 => (89, 159, 198),
        _ => (229, 187, 119),
    };
    (
        Color::Rgb(r, g, b),
        Color::Rgb(r / 4 * 3, g / 5 * 4, b / 4 * 3),
    )
}

fn render_sidebar(frame: &mut Frame, state: &AppState, area: Rect) {
    let put = |frame: &mut Frame, y, text: Line<'_>| {
        if y < area.height {
            write(
                frame,
                Rect::new(area.x + 1, area.y + y, area.width - 2, 1),
                text,
            );
        }
    };
    put(
        frame,
        1,
        Line::from("PATTERN LIBRARY").style(Style::default().fg(MUTED)),
    );
    for (index, pattern) in Pattern::ALL.iter().enumerate() {
        let selected = *pattern == state.selected;
        put(
            frame,
            index as u16 + 3,
            Line::from(format!(
                "{} {}  {}",
                if selected { "▸" } else { " " },
                index + 1,
                pattern.name()
            ))
            .style(Style::default().fg(if selected { MINT } else { MUTED })),
        );
    }
    for (i, line) in state.selected.description().lines().enumerate() {
        put(
            frame,
            11 + i as u16,
            Line::from(line).style(Style::default().fg(TEXT)),
        );
    }
    put(
        frame,
        15,
        Line::from("R replace  ↵ stamp here").style(Style::default().fg(CYAN)),
    );
    put(
        frame,
        17,
        Line::from("POPULATION / LAST 80").style(Style::default().fg(MUTED)),
    );
    if area.height >= 24 {
        frame.render_widget(
            Sparkline::default()
                .data(&state.history)
                .style(Style::default().fg(CYAN))
                .max(state.history.iter().copied().max().unwrap_or(1).max(1)),
            Rect::new(area.x + 1, area.y + 19, area.width - 2, 3),
        );
    }
    put(
        frame,
        23,
        Line::from(vec![
            Span::styled(format!("+{:<6}", state.births), Style::default().fg(MINT)),
            Span::styled(
                format!("−{:<6}", state.deaths),
                Style::default().fg(Color::Rgb(180, 125, 170)),
            ),
            Span::styled("last step", Style::default().fg(MUTED)),
        ]),
    );
    put(
        frame,
        25,
        Line::from(format!(
            "{:.2}% of the world alive",
            state.population as f64 * 100.0 / CELL_COUNT as f64
        ))
        .style(Style::default().fg(MUTED)),
    );
    put(
        frame,
        28,
        Line::from("CELL AGE").style(Style::default().fg(MUTED)),
    );
    put(
        frame,
        30,
        Line::from(vec![
            Span::styled("██ ", Style::default().fg(MINT)),
            Span::raw("new  "),
            Span::styled("██ ", Style::default().fg(CYAN)),
            Span::raw("grown  "),
            Span::styled("██ ", Style::default().fg(GOLD)),
            Span::raw("old"),
        ])
        .style(Style::default().fg(MUTED)),
    );
    put(
        frame,
        32,
        Line::from("░ fading memory of life").style(Style::default().fg(Color::Rgb(148, 111, 169))),
    );
}

fn render_help(frame: &mut Frame, area: Rect) {
    let width = 58.min(area.width - 2);
    let height = 20.min(area.height);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("  FIELD NOTES  ")
        .border_style(Style::default().fg(CYAN))
        .style(Style::default().bg(BACKGROUND).fg(TEXT));
    let inner = popup.inner(ratatui::layout::Margin::new(2, 1));
    frame.render_widget(block, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Three neighbors bring a cell to life."),
            Line::from("Two or three keep it alive. All else fades."),
            Line::from(""),
            Line::from("Space       Run / pause     N      Single step"),
            Line::from("+ / -       Speed           Z      Cycle zoom"),
            Line::from("1–6         Select pattern  R      Replace world"),
            Line::from("Enter       Stamp at cursor X      Toggle cell"),
            Line::from("Arrows      Move cursor     WASD   Pan camera"),
            Line::from("Home / 0    Recenter        Wheel  Pan vertically"),
            Line::from("Left drag   Paint cells     Right drag   Erase"),
            Line::from("C           Clear world     F      Random soup"),
            Line::from("B           Edge wrapping   G      Grid dots"),
            Line::from("T           Fading trails   Q      Quit"),
            Line::from(""),
            Line::from("Edits pause time. Patterns clip at world edges."),
            Line::from("Help suspends time. Esc / ? closes these notes."),
        ])
        .style(Style::default().fg(TEXT)),
        inner,
    );
}

fn write<'a>(frame: &mut Frame, area: Rect, line: Line<'a>) {
    frame.render_widget(Paragraph::new(line), area);
}
