use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
};
use tui_dispatch::EventKind;
use tui_dispatch_components::{
    BaseStyle, Modal, ModalBehavior, ModalProps, ModalStyle, Padding, ScrollbarStyle, SelectList,
    SelectListBehavior, SelectListProps, SelectListStyle, SelectionStyle, TextInput,
    TextInputProps, TextInputStyle, centered_rect, highlight_substring,
};

use super::Component;
use crate::action::Action;
use crate::state::Location;

pub struct SearchOverlay {
    input: TextInput,
    list: SelectList,
    modal: Modal,
    was_open: bool,
}

pub struct SearchOverlayProps<'a> {
    pub query: &'a str,
    pub results: &'a [Location],
    pub selected: usize,
    pub is_focused: bool,
    #[allow(unused)]
    pub error: Option<&'a str>,
    // Action constructors
    pub on_query_change: fn(String) -> Action,
    pub on_query_submit: fn(String) -> Action,
    pub on_select: fn(usize) -> Action,
}

impl Default for SearchOverlay {
    fn default() -> Self {
        Self {
            input: TextInput::new(),
            list: SelectList::new(),
            modal: Modal::new(),
            was_open: false,
        }
    }
}

impl SearchOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_open(&mut self, is_open: bool) {
        if is_open && !self.was_open {
            self.reset();
        }
        self.was_open = is_open;
    }

    fn reset(&mut self) {
        self.input = TextInput::new();
        self.list = SelectList::new();
    }

    fn result_items(results: &[Location], query: &str) -> Vec<Line<'static>> {
        let base = Style::default().fg(Color::Reset);
        let highlight = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        results
            .iter()
            .map(|loc| highlight_substring(&loc.name, query, base, highlight))
            .collect()
    }
}

impl Component<Action> for SearchOverlay {
    type Props<'a> = SearchOverlayProps<'a>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = Action> {
        if !props.is_focused {
            return Vec::new();
        }

        let EventKind::Key(key) = event else {
            return Vec::new();
        };

        // Handle special keys first
        match key.code {
            KeyCode::Esc => return vec![Action::SearchClose],
            KeyCode::Enter => {
                // If we have results, confirm selection; otherwise submit query
                if !props.results.is_empty() {
                    return vec![Action::SearchConfirm];
                }
                return vec![(props.on_query_submit)(props.query.to_string())];
            }
            // Up/down always navigate the list (if results exist)
            KeyCode::Down | KeyCode::Up => {
                if !props.results.is_empty() {
                    let items = Self::result_items(props.results, props.query);
                    let list_props = SelectListProps {
                        items: &items,
                        count: items.len(),
                        selected: props.selected,
                        is_focused: true,
                        style: SelectListStyle {
                            base: BaseStyle {
                                border: None,
                                padding: Padding::xy(1, 1),
                                bg: None,
                                fg: None,
                            },
                            selection: SelectionStyle::default(),
                            scrollbar: ScrollbarStyle::default(),
                        },
                        behavior: SelectListBehavior::default(),
                        on_select: props.on_select,
                        render_item: &|item| item.clone(),
                    };
                    return self
                        .list
                        .handle_event(event, list_props)
                        .into_iter()
                        .collect();
                }
                return Vec::new();
            }
            _ => {}
        }

        // All other keys go to the input
        let input_props = TextInputProps {
            value: props.query,
            placeholder: "Search for a city...",
            is_focused: true,
            style: TextInputStyle {
                base: BaseStyle {
                    border: None,
                    padding: Padding::new(1, 0, 1, 0),
                    bg: None,
                    fg: None,
                },
                placeholder_style: None,
                cursor_style: None,
            },
            on_change: props.on_query_change,
            on_submit: props.on_query_submit,
            on_cursor_move: Some(|_| Action::Render),
        };

        self.input
            .handle_event(event, input_props)
            .into_iter()
            .collect()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        if area.width < 20 || area.height < 8 {
            return;
        }

        let SearchOverlay {
            input, list, modal, ..
        } = self;
        let modal_area = centered_rect(60, 12, area);
        let mut render_content = |frame: &mut Frame, content_area: Rect| {
            let chunks = Layout::vertical([
                Constraint::Length(3), // Input
                Constraint::Min(1),    // Results
            ])
            .split(content_area);

            // Input with lighter background
            let input_props = TextInputProps {
                value: props.query,
                placeholder: "Search for a city...",
                is_focused: props.is_focused,
                style: TextInputStyle {
                    base: BaseStyle {
                        border: None,
                        padding: Padding::all(1),
                        bg: Some(Color::Rgb(50, 50, 60)),
                        fg: None,
                    },
                    placeholder_style: None,
                    cursor_style: None,
                },
                on_change: props.on_query_change,
                on_submit: props.on_query_submit,
                on_cursor_move: Some(|_| Action::Render),
            };
            input.render(frame, chunks[0], input_props);

            let items = Self::result_items(props.results, props.query);
            let list_props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: props.selected,
                is_focused: props.is_focused,
                style: SelectListStyle {
                    base: BaseStyle {
                        border: None,
                        padding: Padding::all(1),
                        bg: None,
                        fg: None,
                    },
                    selection: SelectionStyle::default(),
                    scrollbar: ScrollbarStyle::default(),
                },
                behavior: SelectListBehavior::default(),
                on_select: props.on_select,
                render_item: &|item| item.clone(),
            };
            list.render(frame, chunks[1], list_props);
        };

        modal.render(
            frame,
            area,
            ModalProps {
                is_open: true,
                is_focused: props.is_focused,
                area: modal_area,
                style: ModalStyle {
                    base: BaseStyle {
                        bg: Some(Color::Rgb(35, 35, 45)),
                        padding: Padding::default(),
                        border: None,
                        fg: None,
                    },
                    ..Default::default()
                },
                behavior: ModalBehavior::default(),
                on_close: || Action::SearchClose,
                render_content: &mut render_content,
            },
        );
    }
}
