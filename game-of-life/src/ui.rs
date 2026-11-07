use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Alignment, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Sparkline},
    Frame,
};
use std::rc::Rc;
use tui_dispatch::{Component, EventKind, EventOutcome};
use tui_dispatch_components::{Modal, ModalBehavior, ModalProps, ModalStyle};

use crate::{action::Action, patterns::Pattern, state::*};

pub const BACKGROUND: Color = Color::Reset;
pub const FIELD: Color = Color::Reset;
pub const MINT: Color = Color::Rgb(160, 248, 194);
const CYAN: Color = Color::Rgb(101, 202, 218);
const TEXT: Color = Color::Reset;
const MUTED: Color = Color::DarkGray;
const GOLD: Color = Color::Rgb(236, 196, 124);

#[derive(Clone, Copy, Debug)]
pub struct ScreenLayout {
    pub panel: Rect,
    pub field: Rect,
    pub population: Rect,
}

impl ScreenLayout {
    pub fn new(area: Rect) -> Self {
        let width = area.width.saturating_sub(4).min(100);
        let height = area.height.saturating_sub(2).min(40);
        let panel = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let chart_height = if height >= 30 { 2 } else { 1 };
        let chart_width = width.min(72);
        Self {
            panel,
            field: Rect::new(
                panel.x,
                panel.y + 3,
                width,
                height.saturating_sub(10 + chart_height),
            ),
            population: Rect::new(
                panel.x + (width - chart_width) / 2,
                panel.y + height.saturating_sub(6 + chart_height),
                chart_width,
                chart_height,
            ),
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
    /// At 1×, two world rows share one terminal row.
    pub rows_per_line: u16,
}

impl BoardView {
    pub fn new(state: &AppState, field: Rect) -> Self {
        let (cell_width, cell_height) = [(1, 1), (2, 1), (4, 2)][state.zoom];
        let rows_per_line = if state.zoom == 0 { 2 } else { 1 };
        let area = field.inner(ratatui::layout::Margin::new(1, 1));
        let cols = i32::from(area.width / cell_width);
        let rows = i32::from(area.height * rows_per_line / cell_height);
        Self {
            area,
            cols,
            rows,
            cell_width,
            cell_height,
            rows_per_line,
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
        // Terminal mouse events cannot identify a half-row: target the upper
        // world cell at 1×. The keyboard cursor can select either half.
        let y = i32::from((row - self.area.y) * self.rows_per_line / self.cell_height);
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
    let layout = ScreenLayout::new(area);
    let panel = layout.panel;
    let row = |y| Rect::new(panel.x, panel.y + y, panel.width, 1);
    write(
        frame,
        row(0),
        Line::from("Game of Life").centered().style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    );
    write(
        frame,
        row(1),
        Line::from(state.scene.to_uppercase())
            .centered()
            .style(Style::default().fg(MUTED)),
    );
    render_field(frame, state, layout.field);
    render_population(frame, state, layout.population);

    let status = if state.running { "RUNNING" } else { "PAUSED" };
    let info_y = panel.height - 5;
    write(
        frame,
        row(info_y),
        Line::from(vec![
            Span::raw("Gen "),
            Span::styled(
                format!("{:06}", state.generation),
                Style::default().fg(TEXT),
            ),
            Span::raw("  ·  Alive "),
            Span::styled(state.population.to_string(), Style::default().fg(MINT)),
            Span::raw(format!("  ·  {} gen/s  ·  ", state.speed())),
            Span::styled(
                status,
                Style::default().fg(if state.running { MUTED } else { GOLD }),
            ),
        ])
        .centered()
        .style(Style::default().fg(MUTED)),
    );
    let key = |text| Span::styled(text, Style::default().fg(TEXT));
    write(
        frame,
        row(info_y + 2),
        Line::from(vec![
            key("space"),
            Span::raw(" run/pause · "),
            key("n"),
            Span::raw(" step · "),
            key("1–6"),
            Span::raw(" pattern · "),
            key("?"),
            Span::raw(" help · "),
            key("q"),
            Span::raw(" quit"),
        ])
        .centered()
        .style(Style::default().fg(MUTED)),
    );
    write(
        frame,
        row(info_y + 3),
        Line::from(vec![
            Span::styled(state.selected.name(), Style::default().fg(CYAN)),
            Span::raw(" · "),
            key("r"),
            Span::raw(" load · "),
            key("↵"),
            Span::raw(" stamp · "),
            key("z"),
            Span::raw(format!(" zoom {}×", [1, 2, 4][state.zoom])),
        ])
        .centered()
        .style(Style::default().fg(MUTED)),
    );
    if state.show_help {
        render_help(frame, area);
    }
}

fn render_field(frame: &mut Frame, state: &AppState, area: Rect) {
    let view = BoardView::new(state, area);
    if view.rows_per_line == 2 {
        render_half_cells(frame, state, view);
        return;
    }
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
                        if state.zoom == 2 {
                            // A 3×3 half-cell square inside a 4×4 slot leaves
                            // equal horizontal and vertical gutters. Shade the
                            // cell once from top to bottom, without repeating
                            // the same bright/dark stripe on both text rows.
                            let middle = mix_colors(bright, base);
                            if dx == view.cell_width - 1 {
                                cell.set_char(' ').set_bg(FIELD);
                            } else if dy == 0 {
                                cell.set_char('▀').set_fg(bright).set_bg(middle);
                            } else {
                                cell.set_char('▀').set_fg(base).set_bg(FIELD);
                            }
                        } else {
                            cell.set_char('▀').set_fg(bright).set_bg(base);
                        }
                    } else if state.show_trails && trail > 0 {
                        let color = Color::Rgb(23 + trail * 4, 24 + trail * 2, 41 + trail * 5);
                        let glyph = if state.zoom == 2 {
                            if dx == view.cell_width - 1 {
                                ' '
                            } else if dy == 0 {
                                '░'
                            } else {
                                '▀'
                            }
                        } else {
                            '░'
                        };
                        cell.set_char(glyph).set_fg(color).set_bg(FIELD);
                    } else {
                        let dot = state.show_grid && x % 4 == 0 && y % 4 == 0 && dx == 0 && dy == 0;
                        cell.set_char(if dot { '·' } else { ' ' })
                            .set_fg(Color::DarkGray)
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

fn render_half_cells(frame: &mut Frame, state: &AppState, view: BoardView) {
    let color_at = |x, y| {
        let index = AppState::index(x, y)?;
        if state.cursor_visible && (x, y) == (state.cursor_x, state.cursor_y) {
            Some(GOLD)
        } else if state.cells[index] > 0 {
            Some(age_colors(state.cells[index]).0)
        } else if state.show_trails && state.trails[index] > 0 {
            let trail = state.trails[index];
            Some(Color::Rgb(23 + trail * 4, 24 + trail * 2, 41 + trail * 5))
        } else {
            None
        }
    };
    for row in 0..view.area.height {
        for col in 0..view.area.width {
            let x = view.origin_x + i32::from(col);
            let y = view.origin_y + i32::from(row) * 2;
            let (glyph, fg, bg) = match (color_at(x, y), color_at(x, y + 1)) {
                (Some(top), Some(bottom)) => ('▀', top, bottom),
                (Some(top), None) => ('▀', top, FIELD),
                (None, Some(bottom)) => ('▄', bottom, FIELD),
                (None, None) => (
                    if state.show_grid && x % 4 == 0 && (y % 4 == 0 || (y + 1) % 4 == 0) {
                        '·'
                    } else {
                        ' '
                    },
                    Color::DarkGray,
                    FIELD,
                ),
            };
            frame.buffer_mut()[(view.area.x + col, view.area.y + row)]
                .set_char(glyph)
                .set_fg(fg)
                .set_bg(bg);
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

fn mix_colors(a: Color, b: Color) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => Color::Rgb(
            ((u16::from(ar) + u16::from(br)) / 2) as u8,
            ((u16::from(ag) + u16::from(bg)) / 2) as u8,
            ((u16::from(ab) + u16::from(bb)) / 2) as u8,
        ),
        _ => a,
    }
}

fn render_population(frame: &mut Frame, state: &AppState, area: Rect) {
    let label_width = 12.min(area.width);
    write(
        frame,
        Rect::new(area.x, area.bottom() - 1, label_width, 1),
        Line::from("population").style(Style::default().fg(MUTED)),
    );
    let chart = Rect::new(
        area.x + label_width,
        area.y,
        area.width.saturating_sub(label_width),
        area.height,
    );
    // Keep the most recent generations visible when the chart is narrower
    // than the stored history, including after a terminal resize.
    let start = state.history.len().saturating_sub(usize::from(chart.width));
    let history = &state.history[start..];
    frame.render_widget(
        Sparkline::default()
            .data(history)
            .max(history.iter().copied().max().unwrap_or(1).max(1))
            .style(Style::default().fg(CYAN)),
        chart,
    );
}

fn render_help(frame: &mut Frame, area: Rect) {
    let width = 58.min(area.width);
    let height = 21.min(area.height);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let mut content = |frame: &mut Frame, inner: Rect| {
        let inner = inner.inner(Margin::new(2, 1));
        let row = |y| Rect::new(inner.x, inner.y + y, inner.width, 1);
        write(
            frame,
            row(0),
            Line::from("Game of Life").centered().style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        let columns = (inner.width - 2) / 2;
        for (index, ((left_key, left_action), (right_key, right_action))) in [
            (("Space", "Run / pause"), ("N", "Single step")),
            (("+ / -", "Speed"), ("Z", "Zoom")),
            (("Arrows", "Move cursor"), ("WASD", "Pan / wheel")),
            (("Mouse", "Paint / erase"), ("Home / 0", "Recenter")),
            (("Enter", "Stamp pattern"), ("X", "Toggle cell")),
            (("R", "Load pattern"), ("C", "Clear")),
            (("F", "Random soup"), ("B", "Wrap edges")),
            (("G", "Grid"), ("T", "Trails")),
        ]
        .into_iter()
        .enumerate()
        {
            for (column, key, action) in [(0, left_key, left_action), (1, right_key, right_action)]
            {
                write(
                    frame,
                    Rect::new(
                        inner.x + column * (columns + 2),
                        inner.y + index as u16 + 2,
                        columns,
                        1,
                    ),
                    Line::from(vec![
                        Span::styled(format!("{key:<10}"), Style::default().fg(Color::White)),
                        Span::styled(action, Style::default().fg(Color::Gray)),
                    ]),
                );
            }
        }
        write(
            frame,
            row(11),
            Line::from("Patterns").style(Style::default().fg(Color::Gray)),
        );
        for (index, name) in [
            "Garden",
            "Gosper gun",
            "Pulsar",
            "Glider",
            "R-pentomino",
            "Acorn",
        ]
        .into_iter()
        .enumerate()
        {
            let column_width = inner.width / 3;
            write(
                frame,
                Rect::new(
                    inner.x + (index % 3) as u16 * column_width,
                    inner.y + 12 + (index / 3) as u16,
                    column_width,
                    1,
                ),
                Line::from(vec![
                    Span::styled(
                        format!("{}  ", index + 1),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(name, Style::default().fg(Color::Gray)),
                ]),
            );
        }
        write(
            frame,
            row(15),
            Line::from("3 neighbors: birth · 2–3: survive")
                .centered()
                .style(Style::default().fg(Color::Gray)),
        );
        write(
            frame,
            row(17),
            Line::from(vec![
                Span::styled("Esc / ?", Style::default().fg(Color::White)),
                Span::styled("  close     ", Style::default().fg(Color::Gray)),
                Span::styled("Q", Style::default().fg(Color::White)),
                Span::styled("  quit", Style::default().fg(Color::Gray)),
            ])
            .centered(),
        );
    };
    Modal.render(
        frame,
        area,
        ModalProps {
            is_open: true,
            is_focused: true,
            area: popup,
            style: ModalStyle::with_bg(Color::Rgb(35, 35, 45)),
            behavior: ModalBehavior::default(),
            on_close: Rc::new(|| Action::HelpToggle),
            render_content: &mut content,
        },
    );
}

fn write<'a>(frame: &mut Frame, area: Rect, line: Line<'a>) {
    frame.render_widget(Paragraph::new(line), area);
}
