use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};
use std::rc::Rc;
use tui_dispatch::{Component, EventKind};
use tui_dispatch_components::{
    centered_rect, Modal, ModalBehavior, ModalProps, TextInput, TextInputProps,
};

use crate::{
    action::Action,
    form_style,
    state::{AppState, CustomForm},
};

#[derive(Default)]
pub struct Ui {
    inputs: [TextInput; 3],
    modal: Modal,
}

fn input_props(form: &CustomForm, index: usize, replace: bool) -> TextInputProps<'_, Action> {
    let mut style = form_style::input();
    style.base.fg = Some(Color::White);
    // Paint the active input's cursor so focus stays visible in the modal.
    style.cursor_style = Some(Style::default().fg(Color::Black).bg(Color::White));
    TextInputProps {
        value: if replace { "" } else { &form.values[index] },
        placeholder: "",
        is_focused: form.field == index,
        style,
        on_change: Rc::new(Action::CustomValue),
        on_cancel: Some(Rc::new(|_| Action::CustomCancel)),
        on_submit: Rc::new(|_| Action::CustomConfirm),
        on_cursor_move: Some(Rc::new(|_| Action::CustomCursorMove)),
    }
}

fn layout(area: Rect) -> (Rect, [Rect; 7]) {
    let modal = centered_rect(48, 18, area);
    let content = modal.inner(Margin::new(2, 1));
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(content);
    (modal, chunks)
}

pub fn click(area: Rect, x: u16, y: u16, current_field: usize) -> Option<Action> {
    let (_, chunks) = layout(area);
    for (field, row) in chunks[2..5].iter().enumerate() {
        if x >= row.x && x < row.right() && y >= row.y && y < row.bottom() {
            return Some(Action::CustomField(field as i8 - current_field as i8));
        }
    }
    None
}

impl Ui {
    pub fn key(&mut self, event: &EventKind, state: &AppState) -> Vec<Action> {
        let Some(form) = &state.custom_form else {
            return vec![];
        };
        let EventKind::Key(key) = event else {
            return vec![];
        };
        match key.code {
            KeyCode::Esc => return vec![Action::CustomCancel],
            KeyCode::Tab | KeyCode::Down => return vec![Action::CustomField(1)],
            KeyCode::BackTab | KeyCode::Up => return vec![Action::CustomField(-1)],
            KeyCode::Enter => return vec![Action::CustomConfirm],
            KeyCode::Char(c) if !c.is_ascii_digit() => return vec![],
            KeyCode::Char(_)
                if !form.replace && form.values[form.field].len() >= form.max_digits() =>
            {
                return vec![]
            }
            _ => {}
        }
        // Typing replaces a field's initial value; cursor keys and deletion
        // retain the regular TextInput editing behavior used by Weather.
        let replace = form.replace && matches!(key.code, KeyCode::Char(_));
        self.inputs[form.field]
            .handle_event(event, input_props(form, form.field, replace))
            .into_iter()
            .collect()
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState) {
        let Some(form) = &state.custom_form else {
            return;
        };
        if area.width < 28 || area.height < 20 {
            frame.render_widget(Paragraph::new("Resize to edit · esc cancel"), area);
            return;
        }
        let (modal_area, chunks) = layout(area);
        let mut content = |frame: &mut Frame, _: Rect| {
            let text = Style::default().fg(Color::White);
            frame.render_widget(
                Paragraph::new("Custom game").style(text.add_modifier(Modifier::BOLD)),
                chunks[0],
            );
            let labels = [
                "Columns".to_string(),
                "Rows".to_string(),
                format!("Mines ≤{}", form.mine_limit()),
            ];
            for (index, label) in labels.into_iter().enumerate() {
                let [label_area, input_area] =
                    Layout::horizontal([Constraint::Length(13), Constraint::Min(3)])
                        .areas(chunks[index + 2]);
                frame.render_widget(
                    Paragraph::new(label).style(text),
                    Rect::new(label_area.x, label_area.y + 1, label_area.width, 1),
                );
                self.inputs[index].render(
                    frame,
                    Rect::new(input_area.x, input_area.y, input_area.width, 3),
                    input_props(form, index, false),
                );
            }
            frame.render_widget(
                Paragraph::new(form.error.as_deref().unwrap_or(""))
                    .style(Style::default().fg(Color::LightRed)),
                chunks[5],
            );
            frame.render_widget(
                Paragraph::new("↑↓ select · ↵ start · esc cancel").style(text),
                chunks[6],
            );
        };
        self.modal.render(
            frame,
            area,
            ModalProps {
                is_open: true,
                is_focused: true,
                area: modal_area,
                style: form_style::modal(),
                behavior: ModalBehavior::default(),
                on_close: Rc::new(|| Action::CustomCancel),
                render_content: &mut content,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        middleware::MinesweeperMiddleware,
        reducer::reducer,
        state::{Difficulty, GameState},
    };
    use ratatui::{backend::TestBackend, Terminal};
    use tui_dispatch::{prelude::StoreWithMiddleware, testing::key};

    #[test]
    fn custom_fields_support_replacement_editing_and_custom_game_restart() {
        let mut state = AppState::new(Difficulty::Expert);
        let mut ui = Ui::default();
        reducer(&mut state, Action::CustomOpen);
        for input in [
            "1", "2", "Down", "1", "5", "Down", "3", "0", "Left", "9", "Delete",
        ] {
            for action in ui.key(&EventKind::Key(key(input)), &state) {
                reducer(&mut state, action);
            }
        }
        assert_eq!(
            state.custom_form.as_ref().unwrap().values,
            ["12", "15", "39"]
        );
        for action in ui.key(&EventKind::Key(key("Enter")), &state) {
            reducer(&mut state, action);
        }
        assert!(state.custom_form.is_none());
        assert_eq!((state.width, state.height, state.mine_count), (12, 15, 39));
        let mut store = StoreWithMiddleware::new(state, reducer, MinesweeperMiddleware::default());
        let (x, y) = store
            .state()
            .grid
            .iter()
            .enumerate()
            .find_map(|(y, row)| row.iter().position(|c| c.mine).map(|x| (x, y)))
            .unwrap();
        store.dispatch(Action::Reveal(x, y));
        assert!(store.state().game_state != GameState::Lost);
        store.dispatch(Action::NewGame);
        assert_eq!(
            (
                store.state().width,
                store.state().height,
                store.state().mine_count
            ),
            (12, 15, 39)
        );
    }

    #[test]
    fn custom_form_accepts_full_size_and_four_digit_mine_count() {
        let mut state = AppState::new(Difficulty::Expert);
        let mut ui = Ui::default();
        reducer(&mut state, Action::CustomOpen);
        for input in [
            "1", "2", "0", "Down", "1", "2", "0", "Down", "4", "3", "2", "0", "Enter",
        ] {
            for action in ui.key(&EventKind::Key(key(input)), &state) {
                reducer(&mut state, action);
            }
        }
        assert!(state.custom_form.is_none());
        assert_eq!(
            (state.width, state.height, state.mine_count),
            (120, 120, 4320)
        );
        assert_eq!(
            state.grid.iter().flatten().filter(|cell| cell.mine).count(),
            4320
        );
        let (x, y) = state
            .grid
            .iter()
            .enumerate()
            .find_map(|(y, row)| row.iter().position(|cell| cell.mine).map(|x| (x, y)))
            .unwrap();
        reducer(&mut state, Action::Reveal(x, y));
        assert!(state.game_state == GameState::Playing);
        assert!(!state.grid[y][x].mine);
    }

    #[test]
    fn invalid_custom_values_keep_the_board_and_focus_the_invalid_field() {
        for (values, field) in [
            (["", "20", "99"], 0),
            (["121", "20", "99"], 0),
            (["24", "4", "99"], 1),
            (["24", "121", "99"], 1),
            (["24", "20", "0"], 2),
            (["24", "20", "145"], 2),
        ] {
            let mut state = AppState::new(Difficulty::Expert);
            reducer(&mut state, Action::CustomOpen);
            state.custom_form.as_mut().unwrap().values = values.map(String::from);
            reducer(&mut state, Action::CustomConfirm);
            assert_eq!(state.custom_form.as_ref().unwrap().field, field);
            assert!(state.custom_form.as_ref().unwrap().error.is_some());
            assert_eq!((state.width, state.height, state.mine_count), (24, 20, 99));
            reducer(&mut state, Action::CustomCancel);
            assert!(state.custom_form.is_none());
        }
        for (width, height, mines) in [(5, 5, 7), (120, 120, 4320)] {
            let state = AppState::new(Difficulty::Custom {
                width,
                height,
                mines,
            });
            assert_eq!(
                (state.width, state.height, state.mine_count),
                (width, height, mines)
            );
        }
        let state = AppState::new(Difficulty::Custom {
            width: usize::MAX,
            height: usize::MAX,
            mines: usize::MAX,
        });
        assert_eq!(
            (state.width, state.height, state.mine_count),
            (120, 120, 4320)
        );
    }

    #[test]
    fn custom_modal_uses_weather_styles_and_renders_on_small_screens() {
        let mut state = AppState::new(Difficulty::Expert);
        reducer(&mut state, Action::CustomOpen);
        let mut ui = Ui::default();
        for (width, height) in [(100, 50), (40, 33), (20, 8), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| ui.render(frame, frame.area(), &state))
                .unwrap();
            if width >= 40 {
                let cells = &terminal.backend().buffer().content;
                assert!(cells.iter().any(|cell| cell.bg == Color::Rgb(50, 50, 60)));
                assert!(cells.iter().any(|cell| cell.bg == Color::Rgb(35, 35, 45)));
                let text: String = cells.iter().map(|cell| cell.symbol()).collect();
                assert!(text.contains("Mines ≤144"));
            }
        }
    }
}
