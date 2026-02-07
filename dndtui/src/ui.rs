use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use std::sync::OnceLock;
use tui_map::core::TileKind;
use tui_map::render::{Camera, MapRenderResult, MapRenderer, RenderConfig, TextureVariant, TilePalette, TileTheme};
use tui_dispatch::{Component, EventKind, EventOutcome, RenderContext};
use tui_dispatch_components::{
    centered_rect, BaseStyle, BorderStyle, Line as CLine, LinesScroller, Modal, ModalBehavior,
    ModalProps, ModalStyle, Padding, ScrollView, ScrollViewBehavior, ScrollViewProps,
    ScrollViewStyle, SelectList, SelectListBehavior, SelectListProps, SelectListStyle,
    SelectionStyle, StatusBar, StatusBarHint, StatusBarProps, StatusBarSection, StatusBarStyle,
    TextInput, TextInputProps, TextInputStyle,
};

use crate::action::Action;
use crate::icons;
use crate::rules::{BACKGROUND_OPTIONS, CLASS_OPTIONS};
use crate::sprite;
use crate::sprite_backend;
use crate::state::{
    AppState, CreationStep, Direction as MoveDir, GameMode, LogSpeaker, MenuState,
};

const BG_BASE: Color = Color::Rgb(16, 18, 20);
const PANEL_BG: Color = Color::Rgb(26, 28, 32);
const TEXT_MAIN: Color = Color::Rgb(232, 232, 232);
const TEXT_DIM: Color = Color::Rgb(160, 160, 160);
const ACCENT: Color = Color::Rgb(126, 200, 180);
const ACCENT_GOLD: Color = Color::Rgb(222, 196, 120);
const ACCENT_RED: Color = Color::Rgb(204, 90, 90);

const CELL_ASPECT: f32 = 2.0;
const MAP_TILES_V: u16 = 10;
const SPRITE_ID_PLAYER: u32 = 0x1000_0001;
const SPRITE_ID_NPC_PREFIX: u32 = 0x2000_0000;
const SPRITE_ID_ITEM_PREFIX: u32 = 0x3000_0000;
const SPRITE_ID_ENCOUNTER_PREFIX: u32 = 0x4000_0000;
static MAP_RENDERER: OnceLock<MapRenderer> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaneFocus {
    Map,
    Sidebar,
    Log,
    Input,
}

pub struct DndUi {
    menu_list: SelectList,
    pause_list: SelectList,
    inventory_list: SelectList,
    class_list: SelectList,
    background_list: SelectList,
    stats_list: SelectList,
    log_view: ScrollView,
    modal: Modal,
    name_input: TextInput,
    dialogue_input: TextInput,
    custom_input: TextInput,
    status_bar: StatusBar,
    focus: PaneFocus,
}

impl DndUi {
    const GAMEPLAY_FOCUS_ORDER: [PaneFocus; 3] =
        [PaneFocus::Map, PaneFocus::Sidebar, PaneFocus::Log];
    const INPUT_FOCUS_ORDER: [PaneFocus; 2] = [PaneFocus::Input, PaneFocus::Log];

    pub fn new() -> Self {
        Self {
            menu_list: SelectList::new(),
            pause_list: SelectList::new(),
            inventory_list: SelectList::new(),
            class_list: SelectList::new(),
            background_list: SelectList::new(),
            stats_list: SelectList::new(),
            log_view: ScrollView::new(),
            modal: Modal::new(),
            name_input: TextInput::new(),
            dialogue_input: TextInput::new(),
            custom_input: TextInput::new(),
            status_bar: StatusBar::new(),
            focus: PaneFocus::Map,
        }
    }

    fn focus_order_for_mode(mode: GameMode) -> &'static [PaneFocus] {
        match mode {
            GameMode::Exploration | GameMode::Combat => &Self::GAMEPLAY_FOCUS_ORDER,
            GameMode::Dialogue | GameMode::CustomAction => &Self::INPUT_FOCUS_ORDER,
            _ => &Self::GAMEPLAY_FOCUS_ORDER[0..1],
        }
    }

    fn normalize_focus(&mut self, state: &AppState) {
        let order = Self::focus_order_for_mode(state.mode);
        if !order.contains(&self.focus) {
            self.focus = order[0];
        }
    }

    fn cycle_focus(&mut self, state: &AppState, reverse: bool) {
        let order = Self::focus_order_for_mode(state.mode);
        if order.is_empty() {
            return;
        }
        let current = order.iter().position(|f| *f == self.focus).unwrap_or(0);
        let next = if reverse {
            current
                .checked_sub(1)
                .unwrap_or(order.len().saturating_sub(1))
        } else {
            (current + 1) % order.len()
        };
        self.focus = order[next];
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, _ctx: RenderContext) {
        self.normalize_focus(state);
        sprite_backend::clear_sprites();
        frame.render_widget(Block::default().style(Style::default().bg(BG_BASE)), area);
        match state.mode {
            GameMode::MainMenu => {
                render_main_menu(frame, area, state, &mut self.menu_list);
                return;
            }
            GameMode::CharacterCreation => {
                render_character_creation(
                    frame,
                    area,
                    state,
                    &mut self.name_input,
                    &mut self.class_list,
                    &mut self.background_list,
                    &mut self.stats_list,
                );
                return;
            }
            _ => {}
        }

        let layout = main_layout(area);
        let map_focus = self.focus == PaneFocus::Map;
        let sidebar_focus = self.focus == PaneFocus::Sidebar;
        let log_focus = self.focus == PaneFocus::Log;

        render_map(frame, layout.map, state, map_focus);
        render_sidebar(frame, layout.sidebar, state, sidebar_focus);
        render_log(frame, layout.log, state, log_focus, &mut self.log_view);
        render_input(
            frame,
            layout.input,
            state,
            self.focus,
            &mut self.dialogue_input,
            &mut self.custom_input,
            &mut self.status_bar,
        );

        if state.pause_menu.is_open {
            render_pause_menu(frame, area, state, &mut self.modal, &mut self.pause_list);
        } else if state.mode == GameMode::Inventory {
            render_inventory_modal(
                frame,
                area,
                state,
                &mut self.modal,
                &mut self.inventory_list,
            );
        }
    }

    pub fn handle_event(&mut self, event: &EventKind, state: &AppState) -> EventOutcome<Action> {
        match event {
            EventKind::Resize(width, height) => {
                EventOutcome::action(Action::UiTerminalResize(*width, *height)).with_render()
            }
            EventKind::Key(key) => self.handle_key(*key, event, state),
            EventKind::Scroll { delta, .. } => {
                if *delta == 0 {
                    EventOutcome::ignored()
                } else if !self.can_scroll_log(state) {
                    EventOutcome::ignored()
                } else {
                    let step = (*delta).signum() as i16;
                    EventOutcome::action(Action::ScrollLog(step))
                }
            }
            _ => EventOutcome::ignored(),
        }
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
        event: &EventKind,
        state: &AppState,
    ) -> EventOutcome<Action> {
        if key.kind == KeyEventKind::Release {
            return EventOutcome::ignored();
        }

        if state.pause_menu.is_open {
            return self.handle_pause_event(event, state);
        }
        if state.mode == GameMode::Inventory {
            return self.handle_inventory_event(event, state);
        }
        if state.mode == GameMode::MainMenu {
            return self.handle_menu_key(key, event, state);
        }
        if state.mode == GameMode::CharacterCreation {
            return self.handle_creation_event(event, state);
        }
        if is_tab_key(key) && key.kind == KeyEventKind::Press {
            let reverse = key.modifiers.contains(KeyModifiers::SHIFT);
            self.cycle_focus(state, reverse);
            return EventOutcome::action(Action::UiRender);
        }
        if self.can_scroll_log(state) {
            let outcome = handle_log_scroll_key(key);
            if !outcome.actions.is_empty() {
                return outcome;
            }
        }

        match state.mode {
            GameMode::Dialogue => self.handle_dialogue_event(event, state),
            GameMode::CustomAction => self.handle_custom_action_event(event, state),
            GameMode::Combat => handle_combat_key(key, self.focus),
            GameMode::Exploration => handle_exploration_key(key, self.focus),
            _ => EventOutcome::ignored(),
        }
    }

    fn can_scroll_log(&self, state: &AppState) -> bool {
        if state.pause_menu.is_open || state.mode == GameMode::Inventory {
            return false;
        }
        self.focus == PaneFocus::Log
            && matches!(
                state.mode,
                GameMode::Exploration
                    | GameMode::Combat
                    | GameMode::Dialogue
                    | GameMode::CustomAction
            )
    }

    fn handle_menu_key(
        &mut self,
        key: KeyEvent,
        event: &EventKind,
        state: &AppState,
    ) -> EventOutcome<Action> {
        let Some(menu) = state.menu.as_ref() else {
            return EventOutcome::ignored();
        };
        if key.kind != KeyEventKind::Press {
            return EventOutcome::ignored();
        }

        match key.code {
            KeyCode::Esc => return EventOutcome::action(Action::Quit),
            KeyCode::Enter => {
                let quit_index = if menu.has_save { 2 } else { 1 };
                if menu.selected == quit_index {
                    return EventOutcome::action(Action::Quit);
                }
                return EventOutcome::action(Action::MenuConfirm);
            }
            KeyCode::Char('w') => {
                let new_idx = if menu.selected == 0 {
                    if menu.has_save {
                        2
                    } else {
                        1
                    }
                } else {
                    menu.selected - 1
                };
                return EventOutcome::action(Action::MenuSelect(new_idx));
            }
            KeyCode::Char('s') => {
                let max = if menu.has_save { 2 } else { 1 };
                let new_idx = if menu.selected >= max {
                    0
                } else {
                    menu.selected + 1
                };
                return EventOutcome::action(Action::MenuSelect(new_idx));
            }
            _ => {}
        }

        let items = menu_items(menu);
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: menu.selected.min(items.len().saturating_sub(1)),
            is_focused: true,
            style: menu_list_style(),
            behavior: SelectListBehavior {
                show_scrollbar: false,
                wrap_navigation: true,
            },
            on_select: Action::MenuSelect,
            render_item: &render_line,
        };
        EventOutcome::from_actions(self.menu_list.handle_event(event, props))
    }

    fn handle_pause_event(&mut self, event: &EventKind, state: &AppState) -> EventOutcome<Action> {
        let full_area = full_area(state);
        let modal_area = pause_modal_area(full_area);
        let mut noop_render = |_frame: &mut Frame, _area: Rect| {};
        let modal_props = ModalProps {
            is_open: true,
            is_focused: true,
            area: modal_area,
            style: pause_modal_style(),
            behavior: ModalBehavior {
                close_on_esc: true,
                close_on_backdrop: false,
            },
            on_close: pause_close,
            render_content: &mut noop_render,
        };

        let modal_actions: Vec<_> = self
            .modal
            .handle_event(event, modal_props)
            .into_iter()
            .collect();
        if !modal_actions.is_empty() {
            return EventOutcome::actions(modal_actions);
        }

        let items = pause_items();

        if let EventKind::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return EventOutcome::ignored();
            }
            match key.code {
                KeyCode::Enter => return EventOutcome::action(Action::PauseConfirm),
                KeyCode::Char('w') => {
                    let next = if state.pause_menu.selected == 0 {
                        items.len().saturating_sub(1)
                    } else {
                        state.pause_menu.selected - 1
                    };
                    return EventOutcome::action(Action::PauseSelect(next));
                }
                KeyCode::Char('s') => {
                    let max = items.len().saturating_sub(1);
                    let next = if state.pause_menu.selected >= max {
                        0
                    } else {
                        state.pause_menu.selected + 1
                    };
                    return EventOutcome::action(Action::PauseSelect(next));
                }
                _ => {}
            }
        }

        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: state.pause_menu.selected.min(items.len().saturating_sub(1)),
            is_focused: true,
            style: menu_list_style(),
            behavior: SelectListBehavior {
                show_scrollbar: false,
                wrap_navigation: true,
            },
            on_select: Action::PauseSelect,
            render_item: &render_line,
        };

        EventOutcome::from_actions(self.pause_list.handle_event(event, props))
    }

    fn handle_inventory_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> EventOutcome<Action> {
        let modal_area = inventory_modal_area(full_area(state));
        let mut noop_render = |_frame: &mut Frame, _area: Rect| {};
        let modal_props = ModalProps {
            is_open: true,
            is_focused: true,
            area: modal_area,
            style: inventory_modal_style(),
            behavior: ModalBehavior {
                close_on_esc: true,
                close_on_backdrop: false,
            },
            on_close: inventory_close,
            render_content: &mut noop_render,
        };

        let modal_actions: Vec<_> = self
            .modal
            .handle_event(event, modal_props)
            .into_iter()
            .collect();
        if !modal_actions.is_empty() {
            return EventOutcome::actions(modal_actions);
        }

        if let EventKind::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    _ if is_tab_key(*key) => return EventOutcome::action(Action::CloseOverlay),
                    KeyCode::Char('w') => {
                        let len = state.player.inventory.len();
                        if len > 0 {
                            let next = if state.inventory_selected == 0 {
                                len.saturating_sub(1)
                            } else {
                                state.inventory_selected.saturating_sub(1)
                            };
                            return EventOutcome::action(Action::InventorySelect(next));
                        }
                    }
                    KeyCode::Char('s') => {
                        let len = state.player.inventory.len();
                        if len > 0 {
                            let last = len.saturating_sub(1);
                            let next = if state.inventory_selected >= last {
                                0
                            } else {
                                state.inventory_selected + 1
                            };
                            return EventOutcome::action(Action::InventorySelect(next));
                        }
                    }
                    _ => {}
                }
            }
        }

        let items = inventory_items(state);
        if items.is_empty() {
            return EventOutcome::ignored();
        }

        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: state.inventory_selected.min(items.len().saturating_sub(1)),
            is_focused: true,
            style: inventory_list_style(),
            behavior: SelectListBehavior {
                show_scrollbar: items.len() > 8,
                wrap_navigation: true,
            },
            on_select: Action::InventorySelect,
            render_item: &render_line,
        };
        EventOutcome::from_actions(self.inventory_list.handle_event(event, props))
    }

    fn handle_creation_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> EventOutcome<Action> {
        match state.creation.step {
            CreationStep::Name => {
                if let EventKind::Key(key) = event {
                    if key.code == KeyCode::Esc && key.kind == KeyEventKind::Press {
                        return EventOutcome::action(Action::Quit);
                    }
                }
                let props = TextInputProps {
                    value: &state.creation.name,
                    placeholder: "Name your adventurer",
                    is_focused: true,
                    style: input_style(),
                    on_change: Action::CreationNameChanged,
                    on_submit: submit_creation_name,
                    on_cursor_move: Some(ui_render),
                };
                EventOutcome::from_actions(self.name_input.handle_event(event, props))
            }
            CreationStep::Class => {
                let items = list_items(CLASS_OPTIONS);
                handle_creation_list_event(
                    event,
                    state.creation.class_index,
                    &items,
                    Action::CreationSelectClass,
                    &mut self.class_list,
                )
            }
            CreationStep::Background => {
                let items = list_items(BACKGROUND_OPTIONS);
                handle_creation_list_event(
                    event,
                    state.creation.background_index,
                    &items,
                    Action::CreationSelectBackground,
                    &mut self.background_list,
                )
            }
            CreationStep::Stats => {
                let items = creation_stat_items(state);
                if let EventKind::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Left => {
                                return EventOutcome::action(Action::CreationAdjustStat(-1))
                            }
                            KeyCode::Right => {
                                return EventOutcome::action(Action::CreationAdjustStat(1))
                            }
                            KeyCode::Enter => return EventOutcome::action(Action::CreationNext),
                            KeyCode::Backspace => {
                                return EventOutcome::action(Action::CreationBack)
                            }
                            KeyCode::Char('w') => {
                                let next = if state.creation.selected_stat == 0 {
                                    items.len().saturating_sub(1)
                                } else {
                                    state.creation.selected_stat - 1
                                };
                                return EventOutcome::action(Action::CreationSelectStat(next));
                            }
                            KeyCode::Char('s') => {
                                let max = items.len().saturating_sub(1);
                                let next = if state.creation.selected_stat >= max {
                                    0
                                } else {
                                    state.creation.selected_stat + 1
                                };
                                return EventOutcome::action(Action::CreationSelectStat(next));
                            }
                            _ => {}
                        }
                    }
                }

                let props = SelectListProps {
                    items: &items,
                    count: items.len(),
                    selected: state
                        .creation
                        .selected_stat
                        .min(items.len().saturating_sub(1)),
                    is_focused: true,
                    style: creation_list_style(),
                    behavior: SelectListBehavior {
                        show_scrollbar: false,
                        wrap_navigation: true,
                    },
                    on_select: Action::CreationSelectStat,
                    render_item: &render_line,
                };
                EventOutcome::from_actions(self.stats_list.handle_event(event, props))
            }
            CreationStep::Confirm => match event {
                EventKind::Key(key) => match key.code {
                    KeyCode::Enter => EventOutcome::action(Action::CreationConfirm),
                    KeyCode::Backspace => EventOutcome::action(Action::CreationBack),
                    _ => EventOutcome::ignored(),
                },
                _ => EventOutcome::ignored(),
            },
        }
    }

    fn handle_dialogue_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> EventOutcome<Action> {
        if let EventKind::Key(key) = event {
            if key.code == KeyCode::Esc && key.kind == KeyEventKind::Press {
                return EventOutcome::action(Action::CloseOverlay);
            }
            if self.focus == PaneFocus::Log {
                return handle_log_scroll_key(*key);
            }
        }

        let props = TextInputProps {
            value: &state.dialogue.input,
            placeholder: "Speak to the DM...",
            is_focused: self.focus == PaneFocus::Input,
            style: input_style(),
            on_change: Action::DialogueInputChanged,
            on_submit: submit_dialogue,
            on_cursor_move: Some(ui_render),
        };
        EventOutcome::from_actions(self.dialogue_input.handle_event(event, props))
    }

    fn handle_custom_action_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> EventOutcome<Action> {
        if let EventKind::Key(key) = event {
            if key.code == KeyCode::Esc && key.kind == KeyEventKind::Press {
                return EventOutcome::action(Action::CloseOverlay);
            }
            if self.focus == PaneFocus::Log {
                return handle_log_scroll_key(*key);
            }
        }

        let props = TextInputProps {
            value: &state.custom_action.input,
            placeholder: "Describe your action...",
            is_focused: self.focus == PaneFocus::Input,
            style: input_style(),
            on_change: Action::CustomActionInputChanged,
            on_submit: submit_custom_action,
            on_cursor_move: Some(ui_render),
        };
        EventOutcome::from_actions(self.custom_input.handle_event(event, props))
    }
}

struct UiLayout {
    map: Rect,
    sidebar: Rect,
    log: Rect,
    input: Rect,
}

fn main_layout(area: Rect) -> UiLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),
            Constraint::Length(7),
            Constraint::Length(3),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
        .split(vertical[0]);

    UiLayout {
        map: top[0],
        sidebar: top[1],
        log: vertical[1],
        input: vertical[2],
    }
}

fn full_area(state: &AppState) -> Rect {
    Rect::new(0, 0, state.terminal_size.0, state.terminal_size.1)
}

fn menu_option_labels(menu: &MenuState) -> Vec<&'static str> {
    if menu.has_save {
        vec!["New Game", "Continue", "Quit"]
    } else {
        vec!["New Game", "Quit"]
    }
}

fn pause_option_labels() -> Vec<&'static str> {
    vec!["Resume", "Save Game", "Quit to Menu"]
}

fn menu_items(menu: &MenuState) -> Vec<CLine<'static>> {
    let options = menu_option_labels(menu);
    list_items(&options)
}

fn pause_items() -> Vec<CLine<'static>> {
    let options = pause_option_labels();
    list_items(&options)
}

fn inventory_items(state: &AppState) -> Vec<CLine<'static>> {
    state
        .player
        .inventory
        .iter()
        .map(|item| CLine::from(format!("{:<22} x{}", item.name, item.qty)))
        .collect()
}

fn list_items(items: &[&str]) -> Vec<CLine<'static>> {
    items
        .iter()
        .map(|item| CLine::from(item.to_string()))
        .collect()
}

fn centered_items(items: &[&str], width: u16) -> Vec<CLine<'static>> {
    let width = width.max(1) as usize;
    items
        .iter()
        .map(|item| {
            let padded = format!("{:^width$}", item, width = width);
            CLine::from(padded)
        })
        .collect()
}

fn render_line(item: &CLine<'static>) -> CLine<'static> {
    item.clone()
}

fn creation_step_label(step: CreationStep) -> (usize, &'static str) {
    match step {
        CreationStep::Name => (1, "Name"),
        CreationStep::Class => (2, "Class"),
        CreationStep::Background => (3, "Background"),
        CreationStep::Stats => (4, "Stats"),
        CreationStep::Confirm => (5, "Confirm"),
    }
}

fn creation_footer_text(step: CreationStep) -> &'static str {
    match step {
        CreationStep::Name => "Enter: Next  •  Esc: Quit",
        CreationStep::Class | CreationStep::Background => {
            "↑/↓ or W/S: Choose  •  Enter: Next  •  Backspace: Back"
        }
        CreationStep::Stats => {
            "↑/↓ or W/S: Select  •  ←/→: Adjust  •  Enter: Next  •  Backspace: Back"
        }
        CreationStep::Confirm => "Enter: Confirm  •  Backspace: Back",
    }
}

fn creation_stat_items(state: &AppState) -> Vec<CLine<'static>> {
    let stats = [
        ("Strength", state.creation.stats.strength),
        ("Dexterity", state.creation.stats.dexterity),
        ("Constitution", state.creation.stats.constitution),
        ("Intelligence", state.creation.stats.intelligence),
        ("Wisdom", state.creation.stats.wisdom),
        ("Charisma", state.creation.stats.charisma),
    ];

    stats
        .iter()
        .map(|(label, score)| CLine::from(format!("{label:<12} {score:>2}")))
        .collect()
}

fn pause_modal_area(area: Rect) -> Rect {
    centered_rect(32, 11, area)
}

fn inventory_modal_area(area: Rect) -> Rect {
    centered_rect(56, 16, area)
}

fn panel_border_style() -> BorderStyle {
    BorderStyle {
        borders: Borders::ALL,
        style: Style::default().fg(TEXT_DIM),
        focused_style: Some(Style::default().fg(ACCENT)),
    }
}

fn panel_block<'a>(title: &'a str, is_focused: bool) -> Block<'a> {
    let border = panel_border_style();
    Block::default()
        .title(title)
        .borders(border.borders)
        .border_style(border.style_for_focus(is_focused))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL_BG))
}

fn panel_block_for_mode<'a>(title: &'a str, is_focused: bool, mode: GameMode) -> Block<'a> {
    let mut block = panel_block(title, is_focused);
    if mode == GameMode::Combat {
        let border = if is_focused {
            ACCENT_RED
        } else {
            Color::Rgb(136, 78, 78)
        };
        block = block.border_style(Style::default().fg(border));
    }
    block
}

fn pause_modal_style() -> ModalStyle {
    ModalStyle {
        dim_factor: 0.6,
        base: BaseStyle {
            border: Some(BorderStyle {
                borders: Borders::ALL,
                style: Style::default().fg(TEXT_DIM),
                focused_style: Some(Style::default().fg(ACCENT_GOLD)),
            }),
            padding: Padding::all(1),
            bg: Some(PANEL_BG),
            fg: None,
        },
    }
}

fn inventory_modal_style() -> ModalStyle {
    ModalStyle {
        dim_factor: 0.6,
        base: BaseStyle {
            border: Some(BorderStyle {
                borders: Borders::ALL,
                style: Style::default().fg(TEXT_DIM),
                focused_style: Some(Style::default().fg(ACCENT)),
            }),
            padding: Padding::all(1),
            bg: Some(PANEL_BG),
            fg: None,
        },
    }
}

fn pause_close() -> Action {
    Action::PauseClose
}

fn inventory_close() -> Action {
    Action::CloseOverlay
}

fn menu_list_style() -> SelectListStyle {
    SelectListStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(PANEL_BG),
            fg: Some(TEXT_MAIN),
        },
        selection: SelectionStyle::style_only(
            Style::default()
                .fg(ACCENT_GOLD)
                .add_modifier(Modifier::BOLD),
        ),
        scrollbar: Default::default(),
    }
}

fn inventory_list_style() -> SelectListStyle {
    SelectListStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(PANEL_BG),
            fg: Some(TEXT_MAIN),
        },
        selection: SelectionStyle {
            style: Some(
                Style::default()
                    .fg(ACCENT_GOLD)
                    .add_modifier(Modifier::BOLD),
            ),
            marker: Some("› "),
            disabled: false,
        },
        scrollbar: Default::default(),
    }
}

fn creation_list_style() -> SelectListStyle {
    SelectListStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(PANEL_BG),
            fg: Some(TEXT_MAIN),
        },
        selection: SelectionStyle {
            style: Some(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            marker: Some("> "),
            disabled: false,
        },
        scrollbar: Default::default(),
    }
}

fn input_style() -> TextInputStyle {
    TextInputStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(PANEL_BG),
            fg: Some(TEXT_MAIN),
        },
        placeholder_style: Some(Style::default().fg(TEXT_DIM)),
        cursor_style: Some(Style::default().bg(ACCENT_GOLD).fg(BG_BASE)),
    }
}

fn log_scroll_style() -> ScrollViewStyle {
    ScrollViewStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(PANEL_BG),
            fg: Some(TEXT_MAIN),
        },
        scrollbar: Default::default(),
    }
}

fn status_bar_style() -> StatusBarStyle {
    StatusBarStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(PANEL_BG),
            fg: None,
        },
        text: Style::default().fg(TEXT_MAIN),
        hint_key: Style::default()
            .fg(ACCENT_GOLD)
            .add_modifier(Modifier::BOLD),
        hint_label: Style::default().fg(TEXT_DIM),
        separator: Style::default().fg(TEXT_DIM),
    }
}

struct StatusHints {
    left: Vec<StatusBarHint<'static>>,
    center: Vec<StatusBarHint<'static>>,
    right: Vec<StatusBarHint<'static>>,
}

fn status_hints(state: &AppState, focus: PaneFocus) -> StatusHints {
    let hint = |key: &'static str, label: &'static str| StatusBarHint::new(key, label);
    match state.mode {
        GameMode::Exploration if focus == PaneFocus::Log => StatusHints {
            left: vec![hint("Up/Down", "Scroll"), hint("PgUp/Dn", "Page")],
            center: vec![hint("Home/End", "Jump")],
            right: vec![hint("Tab", "Focus"), hint("Esc", "Pause")],
        },
        GameMode::Exploration if focus == PaneFocus::Sidebar => StatusHints {
            left: vec![hint("Tab", "Focus")],
            center: vec![hint("B", "Inventory")],
            right: vec![hint("Esc", "Pause"), hint("PgUp/Dn", "Log")],
        },
        GameMode::Exploration => StatusHints {
            left: vec![hint("Arrows", "Move"), hint("WASD", "Alt Move")],
            center: vec![hint("E", "Interact"), hint("T", "Talk")],
            right: vec![
                hint("Tab", "Focus"),
                hint("B", "Inventory"),
                hint("Esc", "Pause"),
            ],
        },
        GameMode::Combat if focus == PaneFocus::Log => StatusHints {
            left: vec![hint("Up/Down", "Scroll"), hint("PgUp/Dn", "Page")],
            center: vec![hint("Home/End", "Jump")],
            right: vec![hint("Tab", "Focus"), hint("Esc", "Pause")],
        },
        GameMode::Combat if focus == PaneFocus::Sidebar => StatusHints {
            left: vec![hint("Tab", "Focus")],
            center: vec![hint("PgUp/Dn", "Log")],
            right: vec![hint("Esc", "Pause")],
        },
        GameMode::Combat => StatusHints {
            left: vec![hint("Arrows", "Move"), hint("WASD", "Alt Move")],
            center: vec![hint("F/Enter", "Attack"), hint("E", "End Turn")],
            right: vec![hint("Tab", "Focus"), hint("Esc", "Pause")],
        },
        GameMode::Inventory => StatusHints {
            left: vec![hint("Arrows", "Select"), hint("Esc/Tab", "Close")],
            center: Vec::new(),
            right: vec![hint("PgUp/Dn", "Log")],
        },
        GameMode::Dialogue if focus == PaneFocus::Log => StatusHints {
            left: vec![hint("Up/Down", "Scroll"), hint("PgUp/Dn", "Page")],
            center: Vec::new(),
            right: vec![hint("Tab", "Focus"), hint("Esc", "Close")],
        },
        GameMode::Dialogue => StatusHints {
            left: vec![hint("Enter", "Send"), hint("Esc", "Close")],
            center: vec![hint("Tab", "Focus")],
            right: vec![hint("PgUp/Dn", "Log")],
        },
        GameMode::CustomAction if focus == PaneFocus::Log => StatusHints {
            left: vec![hint("Up/Down", "Scroll"), hint("PgUp/Dn", "Page")],
            center: Vec::new(),
            right: vec![hint("Tab", "Focus"), hint("Esc", "Close")],
        },
        GameMode::CustomAction => StatusHints {
            left: vec![hint("Enter", "Submit"), hint("Esc", "Close")],
            center: vec![hint("Tab", "Focus")],
            right: vec![hint("PgUp/Dn", "Log")],
        },
        _ => StatusHints {
            left: Vec::new(),
            center: Vec::new(),
            right: Vec::new(),
        },
    }
}

fn ui_render(_: usize) -> Action {
    Action::UiRender
}

fn submit_creation_name(_: String) -> Action {
    Action::CreationNext
}

fn submit_dialogue(_: String) -> Action {
    Action::DialogueSubmit
}

fn submit_custom_action(_: String) -> Action {
    Action::CustomActionSubmit
}

fn scroll_log_to(_: usize) -> Action {
    Action::UiRender
}

fn is_tab_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Tab
        || (key.code == KeyCode::Char('i') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn is_inventory_open_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('b')
        && key.kind == KeyEventKind::Press
        && key.modifiers == KeyModifiers::NONE
}

fn handle_creation_list_event(
    event: &EventKind,
    selected: usize,
    items: &[CLine<'static>],
    on_select: fn(usize) -> Action,
    list: &mut SelectList,
) -> EventOutcome<Action> {
    if let EventKind::Key(key) = event {
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Enter => return EventOutcome::action(Action::CreationNext),
                KeyCode::Backspace => return EventOutcome::action(Action::CreationBack),
                KeyCode::Char('w') => {
                    let next = if selected == 0 {
                        items.len().saturating_sub(1)
                    } else {
                        selected - 1
                    };
                    return EventOutcome::action(on_select(next));
                }
                KeyCode::Char('s') => {
                    let max = items.len().saturating_sub(1);
                    let next = if selected >= max { 0 } else { selected + 1 };
                    return EventOutcome::action(on_select(next));
                }
                _ => {}
            }
        }
    }

    let props = SelectListProps {
        items,
        count: items.len(),
        selected: selected.min(items.len().saturating_sub(1)),
        is_focused: true,
        style: creation_list_style(),
        behavior: SelectListBehavior {
            show_scrollbar: false,
            wrap_navigation: true,
        },
        on_select,
        render_item: &render_line,
    };
    EventOutcome::from_actions(list.handle_event(event, props))
}

fn handle_log_scroll_key(key: KeyEvent) -> EventOutcome<Action> {
    let is_press = key.kind == KeyEventKind::Press;
    let is_repeat = key.kind == KeyEventKind::Repeat;
    match key.code {
        KeyCode::Up | KeyCode::Left | KeyCode::Char('k') | KeyCode::Char('h')
            if is_press || is_repeat =>
        {
            EventOutcome::action(Action::ScrollLog(1))
        }
        KeyCode::Down | KeyCode::Right | KeyCode::Char('j') | KeyCode::Char('l')
            if is_press || is_repeat =>
        {
            EventOutcome::action(Action::ScrollLog(-1))
        }
        KeyCode::PageUp if is_press => EventOutcome::action(Action::ScrollLog(2)),
        KeyCode::PageDown if is_press => EventOutcome::action(Action::ScrollLog(-2)),
        KeyCode::Home if is_press => EventOutcome::action(Action::ScrollLog(10_000)),
        KeyCode::End if is_press => EventOutcome::action(Action::ScrollLog(-10_000)),
        _ => EventOutcome::ignored(),
    }
}

fn handle_exploration_key(key: KeyEvent, focus: PaneFocus) -> EventOutcome<Action> {
    let is_press = key.kind == KeyEventKind::Press;
    let is_repeat = key.kind == KeyEventKind::Repeat;
    match key.code {
        KeyCode::Esc if is_press => EventOutcome::action(Action::PauseOpen),
        _ if is_inventory_open_key(key) => EventOutcome::action(Action::OpenInventory),
        KeyCode::PageUp if is_press => EventOutcome::action(Action::ScrollLog(2)),
        KeyCode::PageDown if is_press => EventOutcome::action(Action::ScrollLog(-2)),
        _ if focus == PaneFocus::Log => handle_log_scroll_key(key),
        KeyCode::Up | KeyCode::Char('w') if focus == PaneFocus::Map && (is_press || is_repeat) => {
            EventOutcome::action(Action::Move(MoveDir::Up))
        }
        KeyCode::Down | KeyCode::Char('s')
            if focus == PaneFocus::Map && (is_press || is_repeat) =>
        {
            EventOutcome::action(Action::Move(MoveDir::Down))
        }
        KeyCode::Left | KeyCode::Char('a')
            if focus == PaneFocus::Map && (is_press || is_repeat) =>
        {
            EventOutcome::action(Action::Move(MoveDir::Left))
        }
        KeyCode::Right | KeyCode::Char('d')
            if focus == PaneFocus::Map && (is_press || is_repeat) =>
        {
            EventOutcome::action(Action::Move(MoveDir::Right))
        }
        KeyCode::Char('e') if focus == PaneFocus::Map && is_press => {
            EventOutcome::action(Action::Interact)
        }
        KeyCode::Char('t') if focus == PaneFocus::Map && is_press => {
            EventOutcome::action(Action::Talk)
        }
        KeyCode::Char('c') if focus == PaneFocus::Map && is_press => {
            EventOutcome::action(Action::OpenCustomAction)
        }
        _ => EventOutcome::ignored(),
    }
}

fn handle_combat_key(key: KeyEvent, focus: PaneFocus) -> EventOutcome<Action> {
    let is_press = key.kind == KeyEventKind::Press;
    let is_repeat = key.kind == KeyEventKind::Repeat;
    match key.code {
        KeyCode::Esc if is_press => EventOutcome::action(Action::PauseOpen),
        KeyCode::PageUp if is_press => EventOutcome::action(Action::ScrollLog(2)),
        KeyCode::PageDown if is_press => EventOutcome::action(Action::ScrollLog(-2)),
        _ if focus == PaneFocus::Log => handle_log_scroll_key(key),
        KeyCode::Up | KeyCode::Char('w') if focus == PaneFocus::Map && (is_press || is_repeat) => {
            EventOutcome::action(Action::Move(MoveDir::Up))
        }
        KeyCode::Down | KeyCode::Char('s')
            if focus == PaneFocus::Map && (is_press || is_repeat) =>
        {
            EventOutcome::action(Action::Move(MoveDir::Down))
        }
        KeyCode::Left | KeyCode::Char('a')
            if focus == PaneFocus::Map && (is_press || is_repeat) =>
        {
            EventOutcome::action(Action::Move(MoveDir::Left))
        }
        KeyCode::Right | KeyCode::Char('d')
            if focus == PaneFocus::Map && (is_press || is_repeat) =>
        {
            EventOutcome::action(Action::Move(MoveDir::Right))
        }
        KeyCode::Char('f') | KeyCode::Enter if focus == PaneFocus::Map && is_press => {
            EventOutcome::action(Action::CombatAttack)
        }
        KeyCode::Char('e') if focus == PaneFocus::Map && is_press => {
            EventOutcome::action(Action::CombatEndTurn)
        }
        _ => EventOutcome::ignored(),
    }
}

fn render_character_creation(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    name_input: &mut TextInput,
    class_list: &mut SelectList,
    background_list: &mut SelectList,
    stats_list: &mut SelectList,
) {
    let block = panel_block("Character Creation", true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(inner);

    let (step_idx, step_label) = creation_step_label(state.creation.step);
    let header = Paragraph::new(Text::from(vec![Line::from(vec![Span::styled(
        format!("Step {step_idx}/5 • {step_label}"),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )])]))
    .alignment(Alignment::Left);
    frame.render_widget(header, chunks[0]);

    match state.creation.step {
        CreationStep::Name => render_creation_name(frame, chunks[1], state, name_input),
        CreationStep::Class => render_creation_list(
            frame,
            chunks[1],
            "Class",
            CLASS_OPTIONS,
            state.creation.class_index,
            class_list,
            Action::CreationSelectClass,
        ),
        CreationStep::Background => render_creation_list(
            frame,
            chunks[1],
            "Background",
            BACKGROUND_OPTIONS,
            state.creation.background_index,
            background_list,
            Action::CreationSelectBackground,
        ),
        CreationStep::Stats => render_creation_stats(frame, chunks[1], state, stats_list),
        CreationStep::Confirm => render_creation_confirm(frame, chunks[1], state),
    }

    let footer = Paragraph::new(Text::from(vec![Line::from(vec![Span::styled(
        creation_footer_text(state.creation.step),
        Style::default().fg(TEXT_DIM),
    )])]))
    .alignment(Alignment::Left);
    frame.render_widget(footer, chunks[2]);
}

fn render_creation_name(frame: &mut Frame, area: Rect, state: &AppState, input: &mut TextInput) {
    let props = TextInputProps {
        value: &state.creation.name,
        placeholder: "Name your adventurer",
        is_focused: true,
        style: input_style(),
        on_change: Action::CreationNameChanged,
        on_submit: submit_creation_name,
        on_cursor_move: Some(ui_render),
    };
    input.render(frame, area, props);
}

fn render_creation_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[&str],
    selected: usize,
    list: &mut SelectList,
    on_select: fn(usize) -> Action,
) {
    let block = panel_block(title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = list_items(items);
    let props = SelectListProps {
        items: &lines,
        count: lines.len(),
        selected: selected.min(lines.len().saturating_sub(1)),
        is_focused: true,
        style: creation_list_style(),
        behavior: SelectListBehavior {
            show_scrollbar: false,
            wrap_navigation: true,
        },
        on_select,
        render_item: &render_line,
    };
    list.render(frame, inner, props);
}

fn render_creation_stats(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    stats_list: &mut SelectList,
) {
    let block = panel_block("Stats (Point Buy)", true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    let items = creation_stat_items(state);
    let props = SelectListProps {
        items: &items,
        count: items.len(),
        selected: state
            .creation
            .selected_stat
            .min(items.len().saturating_sub(1)),
        is_focused: true,
        style: creation_list_style(),
        behavior: SelectListBehavior {
            show_scrollbar: false,
            wrap_navigation: true,
        },
        on_select: Action::CreationSelectStat,
        render_item: &render_line,
    };
    stats_list.render(frame, chunks[0], props);

    let points_color = if state.creation.points_remaining < 0 {
        ACCENT_RED
    } else {
        ACCENT_GOLD
    };
    let footer = Paragraph::new(Line::from(Span::styled(
        format!("Points remaining: {}", state.creation.points_remaining),
        Style::default().fg(points_color),
    )))
    .alignment(Alignment::Left);
    frame.render_widget(footer, chunks[1]);
}

fn render_creation_confirm(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "Review",
        Style::default().fg(ACCENT),
    )));
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(format!("Name: {}", state.creation.name)));
    let class_name = CLASS_OPTIONS
        .get(state.creation.class_index)
        .copied()
        .unwrap_or("Adventurer");
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

fn render_main_menu(frame: &mut Frame, area: Rect, state: &AppState, menu_list: &mut SelectList) {
    let block = panel_block(" DNDTUI ", true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(menu) = state.menu.as_ref() else {
        return;
    };

    let content_area = centered_rect(38, 14, inner);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(content_area);

    let header = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "DNDTUI",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "A tabletop tale in the terminal",
            Style::default().fg(TEXT_DIM),
        )),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(header, layout[0]);

    let options = menu_option_labels(menu);
    let lines = centered_items(&options, layout[1].width);
    let props = SelectListProps {
        items: &lines,
        count: lines.len(),
        selected: menu.selected.min(lines.len().saturating_sub(1)),
        is_focused: true,
        style: menu_list_style(),
        behavior: SelectListBehavior {
            show_scrollbar: false,
            wrap_navigation: true,
        },
        on_select: Action::MenuSelect,
        render_item: &render_line,
    };
    menu_list.render(frame, layout[1], props);

    let footer = Paragraph::new(Line::from(Span::styled(
        "Arrows/WASD: Navigate  |  Enter: Select  |  Esc: Quit",
        Style::default().fg(TEXT_DIM),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(footer, layout[2]);
}

fn render_map(frame: &mut Frame, area: Rect, state: &AppState, is_focused: bool) {
    let block = panel_block_for_mode(state.map.name.as_str(), is_focused, state.mode);
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

    let (player_x, player_y) = state.player_pos();
    let render = map_renderer().render_base(
        frame,
        inner,
        &state.map,
        Camera {
            focus_x: player_x,
            focus_y: player_y,
        },
        is_focused,
    );
    if render.view_tiles_h == 0 || render.view_tiles_v == 0 {
        return;
    }

    let buf = frame.buffer_mut();
    let icons = icons::icon_set();
    let use_icons = render.cols_per_tile >= 3 && render.rows_per_tile >= 2;

    if let Some(icon) = icons.item.as_ref().filter(|_| use_icons) {
        for item in &state.items {
            if item.x == player_x && item.y == player_y {
                continue;
            }
            draw_map_sprite(
                icon,
                sprite_id(SPRITE_ID_ITEM_PREFIX, &item.id),
                item.x,
                item.y,
                render,
            );
        }
    } else {
        for item in &state.items {
            draw_marker(buf, item.x, item.y, render, '*', ACCENT_GOLD);
        }
    }

    if let Some(icon) = icons.npc.as_ref().filter(|_| use_icons) {
        for npc in &state.npcs {
            if npc.x == player_x && npc.y == player_y {
                continue;
            }
            draw_map_sprite(
                icon,
                sprite_id(SPRITE_ID_NPC_PREFIX, &npc.id),
                npc.x,
                npc.y,
                render,
            );
        }
    } else {
        for npc in &state.npcs {
            draw_marker(buf, npc.x, npc.y, render, 'N', ACCENT);
        }
    }

    if let Some(icon) = icons.encounter.as_ref().filter(|_| use_icons) {
        for encounter in &state.encounters {
            if encounter.defeated {
                continue;
            }
            if encounter.x == player_x && encounter.y == player_y {
                continue;
            }
            draw_map_sprite(
                icon,
                sprite_id(SPRITE_ID_ENCOUNTER_PREFIX, &encounter.id),
                encounter.x,
                encounter.y,
                render,
            );
        }
    } else {
        for encounter in &state.encounters {
            if encounter.defeated {
                continue;
            }
            draw_marker(buf, encounter.x, encounter.y, render, 'E', ACCENT_RED);
        }
    }

    if let Some(icon) = icons.player.as_ref().filter(|_| use_icons) {
        draw_map_sprite(
            icon,
            SPRITE_ID_PLAYER,
            player_x,
            player_y,
            render,
        );
    } else {
        draw_marker(buf, player_x, player_y, render, '@', TEXT_MAIN);
    }
}

fn draw_marker(
    buf: &mut ratatui::buffer::Buffer,
    map_x: u16,
    map_y: u16,
    render: MapRenderResult,
    ch: char,
    fg: Color,
) {
    if let Some((center_x, center_y)) = render.marker_cell(map_x, map_y) {
        if let Some(cell) = buf.cell_mut((center_x, center_y)) {
            cell.set_fg(fg).set_char(ch);
        }
    }
}

fn draw_map_sprite(
    sprite: &sprite::SpriteData,
    sprite_id: u32,
    map_x: u16,
    map_y: u16,
    render: MapRenderResult,
) {
    if let Some((tile_x, tile_y)) = render.tile_cell_origin(map_x, map_y) {
        let (cols, rows) = sprite_fit_scaled(sprite, render.cols_per_tile, render.rows_per_tile, 0.9);
        let sprite_frame = sprite.frame(0);
        if let Ok(sequence) = sprite::kitty_sequence(sprite_frame, cols, rows, sprite_id) {
            let offset_x = tile_x + render.cols_per_tile.saturating_sub(cols) / 2;
            let offset_y = tile_y + render.rows_per_tile.saturating_sub(rows) / 2;
            sprite_backend::set_sprite(sprite_id, offset_x, offset_y, sequence);
        }
    }
}

fn sprite_id(prefix: u32, id: &str) -> u32 {
    let mut hash = 2166136261u32 ^ prefix;
    for byte in id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
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

fn render_sidebar(frame: &mut Frame, area: Rect, state: &AppState, is_focused: bool) {
    let block = panel_block_for_mode("Status", is_focused, state.mode);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    let row = |label: &str, value: String| {
        Line::from(vec![
            Span::styled(format!("{label:<11}"), Style::default().fg(TEXT_DIM)),
            Span::styled(value, Style::default().fg(TEXT_MAIN)),
        ])
    };

    lines.push(Line::from(Span::styled(
        "Player",
        Style::default()
            .fg(ACCENT_GOLD)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(row("Name", state.player.name.clone()));
    lines.push(row("Class", state.player.class_name.clone()));
    lines.push(row("Background", state.player.background.clone()));
    lines.push(row(
        "HP",
        format!("{}/{}", state.player.hp, state.player.max_hp),
    ));
    lines.push(row(
        "Position",
        format!("{},{}", state.player.x, state.player.y),
    ));
    lines.push(Line::from(Span::raw("")));

    if let Some(combat) = &state.combat {
        if let Some(enemy) = state.encounters.iter().find(|e| e.id == combat.enemy_id) {
            lines.push(Line::from(Span::styled(
                "Combat",
                Style::default().fg(ACCENT_RED).add_modifier(Modifier::BOLD),
            )));
            lines.push(row("Enemy", enemy.name.clone()));
            lines.push(row("Enemy HP", format!("{}", enemy.hp.max(0))));
            lines.push(row("Move left", format!("{}", combat.movement_left)));
            lines.push(Line::from(Span::raw("")));
        }
    }

    if let Some(pending) = &state.pending_llm {
        let label = match pending {
            crate::state::PendingLlm::Dialogue { .. } => "Talking",
            crate::state::PendingLlm::CustomAction => "Interpreting",
        };
        let spinner = spinner_frame(state.spinner_frame);
        lines.push(Line::from(Span::styled(
            format!("DM {spinner} {label}..."),
            Style::default().fg(ACCENT),
        )));
        lines.push(Line::from(Span::raw("")));
    }
    if state.pending_transcript_index.is_some() {
        let spinner = spinner_frame(state.spinner_frame);
        lines.push(Line::from(Span::styled(
            format!("Save {spinner} Writing..."),
            Style::default().fg(ACCENT_GOLD),
        )));
        lines.push(Line::from(Span::raw("")));
    }

    lines.push(Line::from(Span::styled(
        "Inventory",
        Style::default()
            .fg(ACCENT_GOLD)
            .add_modifier(Modifier::BOLD),
    )));
    if state.player.inventory.is_empty() {
        lines.push(Line::from(Span::styled(
            "(empty)",
            Style::default().fg(TEXT_DIM),
        )));
    } else {
        for item in &state.player.inventory {
            lines.push(Line::from(format!("• {} x{}", item.name, item.qty)));
        }
    }

    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_log(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    is_focused: bool,
    log_view: &mut ScrollView,
) {
    let block = panel_block_for_mode("Log", is_focused, state.mode);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    let width = inner.width as usize;
    if state.log.is_empty() {
        lines.push(Line::from(Span::styled(
            "No log entries yet.",
            Style::default().fg(TEXT_DIM),
        )));
    } else {
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
    }

    let view_height = inner.height as usize;
    let total_lines = lines.len();
    let max_start = total_lines.saturating_sub(view_height);
    let offset_from_bottom = state.log_scroll as usize;
    let scroll_offset = max_start.saturating_sub(offset_from_bottom);

    let scroller = LinesScroller::new(&lines);
    let mut render_content = scroller.renderer();
    log_view.render(
        frame,
        inner,
        ScrollViewProps {
            content_height: scroller.content_height(),
            scroll_offset,
            is_focused,
            style: log_scroll_style(),
            behavior: ScrollViewBehavior {
                show_scrollbar: true,
                scroll_step: 1,
                page_step: 0,
            },
            on_scroll: scroll_log_to,
            render_content: &mut render_content,
        },
    );
}

fn render_input(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    focus: PaneFocus,
    dialogue_input: &mut TextInput,
    custom_input: &mut TextInput,
    status_bar: &mut StatusBar,
) {
    let title = match state.mode {
        GameMode::Dialogue => "Dialogue",
        GameMode::CustomAction => "Custom Action",
        _ => "Controls",
    };
    let is_focused = focus == PaneFocus::Input;
    let block = panel_block_for_mode(title, is_focused, state.mode);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match state.mode {
        GameMode::Dialogue => {
            let (input_area, hint_area) = if inner.height >= 2 {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(inner);
                (chunks[0], chunks[1])
            } else {
                (inner, Rect::new(inner.x, inner.y, inner.width, 0))
            };

            let props = TextInputProps {
                value: &state.dialogue.input,
                placeholder: "Speak to the DM...",
                is_focused,
                style: input_style(),
                on_change: Action::DialogueInputChanged,
                on_submit: submit_dialogue,
                on_cursor_move: Some(ui_render),
            };
            dialogue_input.render(frame, input_area, props);

            if hint_area.height > 0 {
                let hints = status_hints(state, focus);
                let props = StatusBarProps {
                    left: StatusBarSection::hints(&hints.left),
                    center: StatusBarSection::hints(&hints.center),
                    right: StatusBarSection::hints(&hints.right),
                    style: status_bar_style(),
                    is_focused: false,
                };
                <StatusBar as Component<Action>>::render(status_bar, frame, hint_area, props);
            }
        }
        GameMode::CustomAction => {
            let (input_area, hint_area) = if inner.height >= 2 {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(inner);
                (chunks[0], chunks[1])
            } else {
                (inner, Rect::new(inner.x, inner.y, inner.width, 0))
            };

            let props = TextInputProps {
                value: &state.custom_action.input,
                placeholder: "Describe your action...",
                is_focused,
                style: input_style(),
                on_change: Action::CustomActionInputChanged,
                on_submit: submit_custom_action,
                on_cursor_move: Some(ui_render),
            };
            custom_input.render(frame, input_area, props);

            if hint_area.height > 0 {
                let hints = status_hints(state, focus);
                let props = StatusBarProps {
                    left: StatusBarSection::hints(&hints.left),
                    center: StatusBarSection::hints(&hints.center),
                    right: StatusBarSection::hints(&hints.right),
                    style: status_bar_style(),
                    is_focused: false,
                };
                <StatusBar as Component<Action>>::render(status_bar, frame, hint_area, props);
            }
        }
        _ => {
            let hints = status_hints(state, focus);
            let props = StatusBarProps {
                left: StatusBarSection::hints(&hints.left),
                center: StatusBarSection::hints(&hints.center),
                right: StatusBarSection::hints(&hints.right),
                style: status_bar_style(),
                is_focused: false,
            };
            <StatusBar as Component<Action>>::render(status_bar, frame, inner, props);
        }
    }
}

fn render_pause_menu(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    modal: &mut Modal,
    pause_list: &mut SelectList,
) {
    let modal_area = pause_modal_area(area);
    let mut render_content = |frame: &mut Frame, inner: Rect| {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(inner);

        let title = Paragraph::new(Line::from(Span::styled(
            "PAUSED",
            Style::default()
                .fg(ACCENT_GOLD)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(title, layout[0]);

        let options = pause_option_labels();
        let lines = centered_items(&options, layout[1].width);
        let props = SelectListProps {
            items: &lines,
            count: lines.len(),
            selected: state.pause_menu.selected.min(lines.len().saturating_sub(1)),
            is_focused: true,
            style: menu_list_style(),
            behavior: SelectListBehavior {
                show_scrollbar: false,
                wrap_navigation: true,
            },
            on_select: Action::PauseSelect,
            render_item: &render_line,
        };
        pause_list.render(frame, layout[1], props);

        let footer = Paragraph::new(Line::from(Span::styled(
            "Enter: Select  |  Esc: Close",
            Style::default().fg(TEXT_DIM),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(footer, layout[2]);
    };

    let props = ModalProps {
        is_open: true,
        is_focused: true,
        area: modal_area,
        style: pause_modal_style(),
        behavior: ModalBehavior {
            close_on_esc: true,
            close_on_backdrop: false,
        },
        on_close: pause_close,
        render_content: &mut render_content,
    };
    modal.render(frame, area, props);
}

fn render_inventory_modal(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    modal: &mut Modal,
    inventory_list: &mut SelectList,
) {
    let modal_area = inventory_modal_area(area);
    let mut render_content = |frame: &mut Frame, inner: Rect| {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(4),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);

        let title = Paragraph::new(Line::from(Span::styled(
            "INVENTORY",
            Style::default()
                .fg(ACCENT_GOLD)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(title, layout[0]);

        let items = inventory_items(state);
        if items.is_empty() {
            let empty = Paragraph::new(Line::from(Span::styled(
                "Your pack is empty.",
                Style::default().fg(TEXT_DIM),
            )))
            .alignment(Alignment::Center);
            frame.render_widget(empty, layout[1]);
        } else {
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: state.inventory_selected.min(items.len().saturating_sub(1)),
                is_focused: true,
                style: inventory_list_style(),
                behavior: SelectListBehavior {
                    show_scrollbar: items.len() > layout[1].height as usize,
                    wrap_navigation: true,
                },
                on_select: Action::InventorySelect,
                render_item: &render_line,
            };
            inventory_list.render(frame, layout[1], props);
        }

        let detail_line = state
            .player
            .inventory
            .get(state.inventory_selected)
            .map(|item| {
                Line::from(vec![
                    Span::styled("Selected: ", Style::default().fg(TEXT_DIM)),
                    Span::styled(item.name.clone(), Style::default().fg(TEXT_MAIN)),
                    Span::styled(format!("  x{}", item.qty), Style::default().fg(ACCENT_GOLD)),
                ])
            })
            .unwrap_or_else(|| {
                Line::from(Span::styled(
                    "Selected: none",
                    Style::default().fg(TEXT_DIM),
                ))
            });
        let detail = Paragraph::new(detail_line).alignment(Alignment::Left);
        frame.render_widget(detail, layout[2]);

        let footer = Paragraph::new(Line::from(Span::styled(
            "↑/↓ or W/S: Select  |  Esc/Tab: Close",
            Style::default().fg(TEXT_DIM),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(footer, layout[3]);
    };

    let props = ModalProps {
        is_open: true,
        is_focused: true,
        area: modal_area,
        style: inventory_modal_style(),
        behavior: ModalBehavior {
            close_on_esc: true,
            close_on_backdrop: false,
        },
        on_close: inventory_close,
        render_content: &mut render_content,
    };
    modal.render(frame, area, props);
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

fn dnd_map_theme() -> TileTheme {
    let grass_base = Color::Rgb(34, 112, 58);
    let grass_alt = Color::Rgb(38, 120, 64);
    let trail_base = Color::Rgb(156, 132, 76);
    let trail_alt = Color::Rgb(150, 128, 74);
    let floor_base = Color::Rgb(90, 90, 94);
    let floor_alt = Color::Rgb(80, 80, 84);
    let wall_base = Color::Rgb(66, 74, 66);
    let wall_alt = Color::Rgb(60, 68, 60);
    let water_base = Color::Rgb(48, 86, 146);
    let water_alt = Color::Rgb(52, 92, 150);

    let grass = TilePalette::new(
        grass_base,
        grass_alt,
        [
            TextureVariant::new('.', adjust_color(grass_base, 10), 6),
            TextureVariant::new('\'', adjust_color(grass_base, 6), 7),
            TextureVariant::new('`', adjust_color(grass_base, -4), 8),
        ],
    );
    let trail = TilePalette::new(
        trail_base,
        trail_alt,
        [
            TextureVariant::new('.', adjust_color(trail_base, 6), 6),
            TextureVariant::new(':', adjust_color(trail_base, 4), 7),
            TextureVariant::new('\'', adjust_color(trail_base, -4), 8),
        ],
    );
    let floor = TilePalette::new(
        floor_base,
        floor_alt,
        [
            TextureVariant::new('.', adjust_color(floor_base, 6), 6),
            TextureVariant::new(':', adjust_color(floor_base, 4), 7),
            TextureVariant::new('\'', adjust_color(floor_base, -4), 8),
        ],
    );
    let wall = TilePalette::new(
        wall_base,
        wall_alt,
        [
            TextureVariant::new('#', adjust_color(wall_base, 10), 8),
            TextureVariant::new('+', adjust_color(wall_base, 6), 9),
            TextureVariant::new('.', adjust_color(wall_base, 4), 10),
        ],
    );
    let water = TilePalette::new(
        water_base,
        water_alt,
        [
            TextureVariant::new('~', adjust_color(water_base, 10), 8),
            TextureVariant::new('-', adjust_color(water_base, 6), 9),
            TextureVariant::new('.', adjust_color(water_base, 4), 10),
        ],
    );

    TileTheme::builder()
        .fallback(grass)
        .tile(TileKind::Grass, grass)
        .tile(TileKind::Trail, trail)
        .tile(TileKind::Floor, floor)
        .tile(TileKind::Sand, trail)
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
            .theme(dnd_map_theme())
            .build()
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn inventory_open_key_is_plain_b_only() {
        assert!(is_inventory_open_key(press(KeyCode::Char('b'))));
        assert!(!is_inventory_open_key(press(KeyCode::Char('i'))));
        assert!(!is_inventory_open_key(KeyEvent::new(
            KeyCode::Char('b'),
            KeyModifiers::SHIFT
        )));
        assert!(!is_inventory_open_key(KeyEvent::new(
            KeyCode::Char('b'),
            KeyModifiers::CONTROL
        )));
    }

    #[test]
    fn exploration_map_keys_do_not_route_to_inventory() {
        let move_actions =
            handle_exploration_key(press(KeyCode::Char('w')), PaneFocus::Map).actions;
        assert_eq!(move_actions, vec![Action::Move(MoveDir::Up)]);

        let inventory_actions =
            handle_exploration_key(press(KeyCode::Char('b')), PaneFocus::Map).actions;
        assert_eq!(inventory_actions, vec![Action::OpenInventory]);
    }

    #[test]
    fn ctrl_i_is_treated_as_tab_focus_key() {
        let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL);
        assert!(is_tab_key(key));
        assert!(!is_inventory_open_key(key));
    }

    #[test]
    fn log_focus_routes_arrow_keys_to_log_scroll() {
        let up_actions = handle_exploration_key(press(KeyCode::Up), PaneFocus::Log).actions;
        assert_eq!(up_actions, vec![Action::ScrollLog(1)]);

        let down_actions = handle_exploration_key(press(KeyCode::Down), PaneFocus::Log).actions;
        assert_eq!(down_actions, vec![Action::ScrollLog(-1)]);
    }
}
