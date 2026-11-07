use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::state::{AppState, Difficulty, GameState};

/// Characters per cell width.
const CELL_W: u16 = 3;

// Keep the cursor visible when a board is larger than a phone-sized terminal.
fn grid_view(area: Rect, state: &AppState) -> (Rect, usize, usize) {
    let columns = (area.width / CELL_W).max(1) as usize;
    let visible_width = columns.min(state.width);
    let visible_height = (area.height as usize).min(state.height);
    let first_x = state
        .cursor_x
        .saturating_sub(visible_width / 2)
        .min(state.width.saturating_sub(visible_width));
    let first_y = state
        .cursor_y
        .saturating_sub(visible_height / 2)
        .min(state.height.saturating_sub(visible_height));
    let width = (visible_width as u16 * CELL_W).min(area.width);
    let height = visible_height as u16;
    (
        Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        ),
        first_x,
        first_y,
    )
}

// Share the layout with pointer input so every visible cell stays clickable.
fn game_layout(area: Rect, state: &AppState) -> [Rect; 8] {
    let width = area.width.min((state.width as u16 * CELL_W).max(38));
    let height = area.height.min(state.height as u16 + 9);
    let panel = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .areas(panel)
}

pub fn cell_at(area: Rect, state: &AppState, x: u16, y: u16) -> Option<(usize, usize)> {
    let [_, _, _, board, _, _, _, _] = game_layout(area, state);
    let (grid, first_x, first_y) = grid_view(board, state);
    if x < grid.x || x >= grid.right() || y < grid.y || y >= grid.bottom() {
        return None;
    }
    Some((
        first_x + ((x - grid.x) / CELL_W) as usize,
        first_y + (y - grid.y) as usize,
    ))
}

pub fn render_in(frame: &mut Frame, area: Rect, state: &AppState) {
    let [title, difficulty, _, board, _, info, _, help] = game_layout(area, state);
    frame.render_widget(
        Paragraph::new("Minesweeper")
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(crate::GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        title,
    );
    frame.render_widget(
        Paragraph::new(match state.difficulty {
            Difficulty::Custom { .. } => format!("Custom · {} × {}", state.width, state.height),
            _ => state.difficulty.label().to_string(),
        })
        .alignment(Alignment::Center)
        .style(Style::default().fg(crate::MUTED)),
        difficulty,
    );
    draw_board(frame, state, board);
    draw_info(frame, state, info);
    draw_help(frame, state, help);
}

fn draw_board(frame: &mut Frame, state: &AppState, area: Rect) {
    let (grid, first_x, first_y) = grid_view(area, state);
    let visible_width = (grid.width / CELL_W) as usize;

    for row in 0..grid.height {
        let y = first_y + row as usize;
        let spans: Vec<Span> = (first_x..first_x + visible_width)
            .map(|x| cell_span(state, x, y))
            .collect();

        let line = Line::from(spans);
        let row_area = Rect {
            x: grid.x,
            y: grid.y + row,
            width: grid.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(line), row_area);
    }
}

fn cell_span(state: &AppState, x: usize, y: usize) -> Span<'static> {
    let cell = &state.grid[y][x];
    let is_cursor = x == state.cursor_x && y == state.cursor_y;

    // On game over, reveal all mines
    let show_mine = cell.mine && state.game_state == GameState::Lost;

    let (text, mut style) = if show_mine && !cell.revealed {
        // Unrevealed mine shown on game over
        (" * ".to_string(), Style::default().fg(Color::Red))
    } else if cell.revealed {
        if cell.mine {
            // The mine you hit
            (
                " * ".to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else if cell.adjacent == 0 {
            ("   ".to_string(), Style::default())
        } else {
            let color = number_color(cell.adjacent);
            (
                format!(" {} ", cell.adjacent),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        }
    } else if cell.flagged {
        (" \u{25a0} ".to_string(), Style::default().fg(Color::Red))
    } else {
        (
            " \u{00b7} ".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };

    if is_cursor {
        style = style
            // A subtle selection remains visible in native terminal palettes.
            .bg(Color::Rgb(50, 50, 60))
            .add_modifier(Modifier::BOLD);
    }

    Span::styled(text, style)
}

fn number_color(n: u8) -> Color {
    match n {
        1 => Color::Blue,
        2 => Color::Green,
        3 => Color::Red,
        4 => Color::Magenta,
        5 => Color::Yellow,
        6 => Color::Cyan,
        7 => Color::Gray,
        8 => Color::DarkGray,
        _ => Color::White,
    }
}

fn draw_info(frame: &mut Frame, state: &AppState, area: Rect) {
    let (status, status_color) = match state.game_state {
        GameState::Playing => ("Playing", crate::MUTED),
        GameState::Won => ("You win!", Color::Green),
        GameState::Lost => ("Game over", Color::Red),
    };
    let line = Line::from(vec![
        Span::raw("Mines "),
        Span::styled(
            state.mine_count.to_string(),
            Style::default().fg(crate::TEXT),
        ),
        Span::raw("  ·  Flags "),
        Span::styled(
            format!("{}/{}", state.flags_placed, state.mine_count),
            Style::default().fg(crate::TEXT),
        ),
        Span::raw("  ·  "),
        Span::styled(status, Style::default().fg(status_color)),
    ]);
    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(Style::default().fg(crate::MUTED)),
        area,
    );
}

fn draw_help(frame: &mut Frame, state: &AppState, area: Rect) {
    let key = |text| Span::styled(text, Style::default().fg(crate::TEXT));
    let mut lines = Vec::new();
    if state.game_state == GameState::Playing {
        lines.push(Line::from(vec![
            key("↑↓←→"),
            Span::raw(" move · "),
            key("space"),
            Span::raw(" reveal · "),
            key("f"),
            Span::raw(" flag"),
        ]));
    }
    lines.push(Line::from(vec![
        key("1/2/3"),
        Span::raw(" difficulty · "),
        key("n"),
        Span::raw(" new game"),
    ]));
    lines.push(Line::from(vec![
        key("c"),
        Span::raw(" custom board · "),
        key("q"),
        Span::raw(" quit"),
    ]));
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(Style::default().fg(crate::MUTED)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn large_custom_board_scrolls_to_all_corners_with_matching_hit_targets() {
        let mut state = AppState::new(Difficulty::Custom {
            width: 120,
            height: 120,
            mines: 4320,
        });
        let area = Rect::new(0, 0, 38, 30);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        for (x, y) in [(0, 0), (119, 0), (0, 119), (119, 119), (60, 60)] {
            state.cursor_x = x;
            state.cursor_y = y;
            let [_, _, _, board, _, _, _, _] = game_layout(area, &state);
            let (grid, first_x, first_y) = grid_view(board, &state);
            assert!(x >= first_x && x < first_x + (grid.width / CELL_W) as usize);
            assert!(y >= first_y && y < first_y + grid.height as usize);
            let column = grid.x + ((x - first_x) as u16 * CELL_W);
            let row = grid.y + (y - first_y) as u16;
            assert_eq!(cell_at(area, &state, column, row), Some((x, y)));
            terminal
                .draw(|frame| render_in(frame, area, &state))
                .unwrap();
        }
    }
}
