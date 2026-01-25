use std::collections::HashMap;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Frame,
};
use tui_dispatch::{Component, EventContext, EventKind, HandlerResponse, RenderContext};
use tui_dispatch_components::style::BorderStyle;
use tui_dispatch_components::{
    BaseStyle, Padding, SelectList, SelectListBehavior, SelectListProps, SelectListStyle,
    SelectionStyle, StatusBar, StatusBarHint, StatusBarItem, StatusBarProps, StatusBarSection,
    StatusBarStyle,
};

use crate::action::Action;
use crate::sprite;
use crate::sprite_backend;
use crate::state::{AppState, PokemonStat};

const BG_BASE: Color = Color::Rgb(12, 18, 28);
const BG_PANEL: Color = Color::Rgb(20, 32, 46);
const BG_PANEL_ALT: Color = Color::Rgb(26, 40, 58);
const BG_HIGHLIGHT: Color = Color::Rgb(28, 92, 110);
const TEXT_MAIN: Color = Color::Rgb(232, 242, 244);
const TEXT_DIM: Color = Color::Rgb(176, 195, 207);
const ACCENT_TEAL: Color = Color::Rgb(72, 204, 184);
const ACCENT_GOLD: Color = Color::Rgb(228, 176, 88);
const CELL_ASPECT: f32 = 2.0;

pub struct PokeUi {
    dex_list: SelectList,
    evolution_list: SelectList,
    move_list: SelectList,
    ability_list: SelectList,
    encounter_list: SelectList,
    status_bar: StatusBar,
}

impl PokeUi {
    pub fn new() -> Self {
        Self {
            dex_list: SelectList::new(),
            evolution_list: SelectList::new(),
            move_list: SelectList::new(),
            ability_list: SelectList::new(),
            encounter_list: SelectList::new(),
            status_bar: StatusBar::new(),
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        render_ctx: RenderContext,
        event_ctx: &mut EventContext<crate::PokeComponentId>,
    ) {
        render_app(
            frame,
            area,
            state,
            render_ctx,
            event_ctx,
            &mut self.dex_list,
            &mut self.evolution_list,
            &mut self.move_list,
            &mut self.ability_list,
            &mut self.encounter_list,
            &mut self.status_bar,
        );
    }

    pub fn handle_evolution_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> HandlerResponse<Action> {
        handle_evolution_event(event, state, &mut self.evolution_list)
    }

    pub fn handle_header_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> HandlerResponse<Action> {
        handle_header_event(event, state)
    }

    pub fn handle_list_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> HandlerResponse<Action> {
        handle_list_event(event, state, &mut self.dex_list)
    }

    pub fn handle_detail_tabs_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> HandlerResponse<Action> {
        handle_detail_tabs_event(
            event,
            state,
            &mut self.move_list,
            &mut self.ability_list,
            &mut self.encounter_list,
        )
    }

    pub fn handle_search_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> HandlerResponse<Action> {
        handle_search_event(event, state)
    }
}

pub fn render_app(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    _render_ctx: RenderContext,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
    dex_list: &mut SelectList,
    evolution_list: &mut SelectList,
    move_list: &mut SelectList,
    ability_list: &mut SelectList,
    encounter_list: &mut SelectList,
    status_bar: &mut StatusBar,
) {
    let base = Block::default().style(Style::default().bg(BG_BASE));
    frame.render_widget(base, area);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, layout[0], state, event_ctx);
    render_body(
        frame,
        layout[1],
        state,
        event_ctx,
        dex_list,
        evolution_list,
        move_list,
        ability_list,
        encounter_list,
    );
    render_footer(frame, layout[2], state, status_bar);
}

pub fn handle_header_event(event: &EventKind, _state: &AppState) -> HandlerResponse<Action> {
    let actions = match event {
        EventKind::Key(key) => match key.code {
            crossterm::event::KeyCode::Char('c') => vec![Action::TypeFilterClear],
            _ => vec![],
        },
        _ => vec![],
    };
    handler_response(actions)
}

pub fn handle_list_event(
    event: &EventKind,
    state: &AppState,
    dex_list: &mut SelectList,
) -> HandlerResponse<Action> {
    let actions = match event {
        EventKind::Key(key) => {
            if key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT)
                && matches!(
                    key.code,
                    crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Down
                )
            {
                if let Some(count) = evolution_stage_count(state) {
                    if count > 0 {
                        let max_index = count.saturating_sub(1);
                        let current = state.evolution_selected_index.min(max_index);
                        let next = match key.code {
                            crossterm::event::KeyCode::Up => current.saturating_sub(1),
                            crossterm::event::KeyCode::Down => (current + 1).min(max_index),
                            _ => current,
                        };
                        return handler_response(vec![Action::EvolutionSelect(next)]);
                    }
                }
                return handler_response(Vec::new());
            }
            match key.code {
                crossterm::event::KeyCode::PageDown => vec![Action::SelectionPage(1)],
                crossterm::event::KeyCode::PageUp => vec![Action::SelectionPage(-1)],
                crossterm::event::KeyCode::Char('f') => vec![Action::ToggleFavorite],
                crossterm::event::KeyCode::Char('t') => vec![Action::ToggleTeam],
                _ => {
                    let items = dex_items(state);
                    let props = SelectListProps {
                        items: &items,
                        count: items.len(),
                        selected: state.selected_index.min(items.len().saturating_sub(1)),
                        is_focused: true,
                        style: dex_list_style(),
                        behavior: SelectListBehavior {
                            show_scrollbar: true,
                            wrap_navigation: false,
                        },
                        on_select: Action::DexSelect,
                        render_item: &|item| item.clone(),
                    };
                    let actions: Vec<_> =
                        dex_list.handle_event(event, props).into_iter().collect();
                    return handler_response(actions);
                }
            }
        }
        EventKind::Scroll { delta, .. } => vec![Action::SelectionMove((*delta * 3) as i16)],
        _ => vec![],
    };
    handler_response(actions)
}

pub fn handle_detail_tabs_event(
    event: &EventKind,
    state: &AppState,
    move_list: &mut SelectList,
    ability_list: &mut SelectList,
    encounter_list: &mut SelectList,
) -> HandlerResponse<Action> {
    let actions = match event {
        EventKind::Key(key) => match key.code {
            crossterm::event::KeyCode::Left | crossterm::event::KeyCode::Char('h') => {
                vec![Action::DetailTabPrev]
            }
            crossterm::event::KeyCode::Right | crossterm::event::KeyCode::Char('l') => {
                vec![Action::DetailTabNext]
            }
            _ => vec![],
        },
        _ => vec![],
    };
    if !actions.is_empty() {
        return handler_response(actions);
    }
    match state.detail_mode {
        crate::state::DetailMode::Move => {
            let items = move_items(state);
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: state.selected_move_index.min(items.len().saturating_sub(1)),
                is_focused: true,
                style: detail_list_style(),
                behavior: SelectListBehavior {
                    show_scrollbar: true,
                    wrap_navigation: false,
                },
                on_select: Action::MoveSelect,
                render_item: &|item| item.clone(),
            };
            let actions: Vec<_> = move_list.handle_event(event, props).into_iter().collect();
            handler_response(actions)
        }
        crate::state::DetailMode::Ability => {
            let items = ability_items(state);
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: state
                    .selected_ability_index
                    .min(items.len().saturating_sub(1)),
                is_focused: true,
                style: detail_list_style(),
                behavior: SelectListBehavior {
                    show_scrollbar: true,
                    wrap_navigation: false,
                },
                on_select: Action::AbilitySelect,
                render_item: &|item| item.clone(),
            };
            let actions: Vec<_> = ability_list
                .handle_event(event, props)
                .into_iter()
                .collect();
            handler_response(actions)
        }
        crate::state::DetailMode::Encounter => {
            let items = encounter_items(state);
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: state
                    .selected_encounter_index
                    .min(items.len().saturating_sub(1)),
                is_focused: true,
                style: detail_list_style(),
                behavior: SelectListBehavior {
                    show_scrollbar: true,
                    wrap_navigation: false,
                },
                on_select: Action::EncounterSelect,
                render_item: &|item| item.clone(),
            };
            let actions: Vec<_> = encounter_list
                .handle_event(event, props)
                .into_iter()
                .collect();
            handler_response(actions)
        }
        crate::state::DetailMode::General | crate::state::DetailMode::Matchup => {
            HandlerResponse::ignored()
        }
    }
}

fn handle_evolution_event(
    event: &EventKind,
    state: &AppState,
    evolution_list: &mut SelectList,
) -> HandlerResponse<Action> {
    let items = evolution_items(state);
    if items.is_empty() {
        return HandlerResponse::ignored();
    }
    let props = SelectListProps {
        items: &items,
        count: items.len(),
        selected: state
            .evolution_selected_index
            .min(items.len().saturating_sub(1)),
        is_focused: true,
        style: evolution_list_style(),
        behavior: SelectListBehavior {
            show_scrollbar: true,
            wrap_navigation: false,
        },
        on_select: Action::EvolutionSelect,
        render_item: &|item| item.clone(),
    };
    let actions: Vec<_> = evolution_list
        .handle_event(event, props)
        .into_iter()
        .collect();
    handler_response(actions)
}

pub fn handle_search_event(event: &EventKind, _state: &AppState) -> HandlerResponse<Action> {
    let actions = match event {
        EventKind::Key(key) => match key.code {
            crossterm::event::KeyCode::Esc => vec![Action::SearchCancel],
            crossterm::event::KeyCode::Enter => vec![Action::SearchSubmit],
            crossterm::event::KeyCode::Backspace => vec![Action::SearchBackspace],
            crossterm::event::KeyCode::Char(ch) => vec![Action::SearchInput(ch)],
            _ => vec![],
        },
        _ => vec![],
    };
    handler_response(actions)
}

fn handler_response(actions: Vec<Action>) -> HandlerResponse<Action> {
    if actions.is_empty() {
        HandlerResponse::ignored()
    } else {
        HandlerResponse {
            actions,
            consumed: true,
            needs_render: false,
        }
    }
}

fn render_header(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
) {
    event_ctx.set_component_area(crate::PokeComponentId::Header, area);
    if state.search.active {
        event_ctx.set_component_area(crate::PokeComponentId::Search, area);
    }
    let title_style = Style::default()
        .fg(ACCENT_TEAL)
        .add_modifier(Modifier::BOLD);
    let filter = state
        .type_filter
        .as_deref()
        .map(|name| name.to_ascii_uppercase())
        .unwrap_or_else(|| "ALL".to_string());
    let search = if state.search.active {
        format!("/{}_", state.search.query)
    } else if state.search.query.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", state.search.query)
    };
    let region = state
        .current_region()
        .map(|region| region.label.as_str())
        .unwrap_or("KANTO");
    let (route_index, route_total) = route_label(state);
    let (seen, caught, total) = region_counts(state);
    let header_text = Text::from(vec![
        Line::from(vec![
            Span::styled(format!("{region} MAP"), title_style),
            Span::raw("  "),
            Span::styled(
                format!("ROUTE {:02}/{:02}", route_index, route_total),
                Style::default().fg(ACCENT_GOLD),
            ),
            Span::raw("  |  Type: "),
            Span::styled(filter, Style::default().fg(ACCENT_GOLD)),
            Span::raw("  |  Search: "),
            Span::styled(search, Style::default().fg(ACCENT_TEAL)),
        ]),
        Line::from(vec![
            Span::raw("Seen: "),
            Span::styled(format!("{seen}/{total}"), Style::default().fg(ACCENT_TEAL)),
            Span::raw("  Caught: "),
            Span::styled(
                format!("{caught}/{total}"),
                Style::default().fg(ACCENT_GOLD),
            ),
            Span::raw("  |  Team: "),
            Span::styled(state.team.join(", "), Style::default().fg(ACCENT_TEAL)),
        ]),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN))
        .border_style(focus_border(state, crate::state::FocusArea::Header))
        .title("POKEDEX");
    let paragraph = Paragraph::new(header_text)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(TEXT_MAIN));
    frame.render_widget(paragraph, area);
}

fn render_body(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
    dex_list: &mut SelectList,
    evolution_list: &mut SelectList,
    move_list: &mut SelectList,
    ability_list: &mut SelectList,
    encounter_list: &mut SelectList,
) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(area);

    render_list(frame, layout[0], state, event_ctx, dex_list);
    render_detail(
        frame,
        layout[1],
        state,
        event_ctx,
        evolution_list,
        move_list,
        ability_list,
        encounter_list,
    );
}

fn render_list(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
    dex_list: &mut SelectList,
) {
    event_ctx.set_component_area(crate::PokeComponentId::DexList, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("DEX")
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN))
        .border_style(focus_border(state, crate::state::FocusArea::DexList));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items = dex_items(state);
    let props = SelectListProps {
        items: &items,
        count: items.len(),
        selected: state.selected_index.min(items.len().saturating_sub(1)),
        is_focused: state.focus == crate::state::FocusArea::DexList,
        style: dex_list_style(),
        behavior: SelectListBehavior {
            show_scrollbar: true,
            wrap_navigation: false,
        },
        on_select: Action::DexSelect,
        render_item: &|item| item.clone(),
    };
    dex_list.render(frame, inner, props);
}

fn render_detail(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
    evolution_list: &mut SelectList,
    move_list: &mut SelectList,
    ability_list: &mut SelectList,
    encounter_list: &mut SelectList,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("DATA")
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(6)])
        .split(inner);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[0]);

    let sprite_area = Rect {
        x: top[0].x,
        y: top[0].y.saturating_add(1),
        width: top[0].width,
        height: top[0].height.saturating_sub(2),
    };
    render_sprite(frame, sprite_area, state);
    render_stats_panel(frame, top[1], state);
    render_secondary(
        frame,
        layout[1],
        state,
        event_ctx,
        evolution_list,
        move_list,
        ability_list,
        encounter_list,
    );
}

fn render_sprite(frame: &mut Frame, area: Rect, state: &AppState) {
    if let Some(name) = state.detail_name.as_ref() {
        if let Some(sprite) = state.sprite_cache.get(name) {
            let (cols, rows) = sprite_fit(sprite, area.width, area.height);
            let sprite_frame = sprite.frame(state.sprite_frame_index);
            if let Ok(sequence) = sprite::kitty_sequence(sprite_frame, cols, rows) {
                let offset_x = area.x.saturating_add(area.width.saturating_sub(cols) / 2);
                let offset_y = area.y.saturating_add(area.height.saturating_sub(rows) / 2);
                sprite_backend::update_sprite(offset_x, offset_y, sequence);
            } else {
                sprite_backend::clear_sprites();
            }
            return;
        }
    }

    sprite_backend::clear_sprites();
    let content = if state.detail_name.is_none() {
        "[select a pokemon]"
    } else if state.sprite_loading {
        "[loading sprite]"
    } else {
        "[no sprite]"
    };

    let paragraph = Paragraph::new(content)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(TEXT_DIM));
    frame.render_widget(paragraph, area);
}

fn render_stats_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let stats = detail_stats(state.current_detail());
    let stats_block = Block::default()
        .borders(Borders::ALL)
        .title("STATS")
        .style(Style::default().fg(TEXT_MAIN));
    frame.render_widget(
        Paragraph::new(stats)
            .block(stats_block)
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn detail_stats(detail: Option<&crate::state::PokemonDetail>) -> Text<'static> {
    let Some(detail) = detail else {
        return Text::from("No stats loaded.");
    };
    let lines = detail
        .stats
        .iter()
        .map(|stat| Line::from(render_stat(stat)))
        .collect::<Vec<_>>();
    Text::from(lines)
}

// profile block removed; details live in General tab

fn render_secondary(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
    evolution_list: &mut SelectList,
    move_list: &mut SelectList,
    ability_list: &mut SelectList,
    encounter_list: &mut SelectList,
) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_detail_tabs(
        frame,
        layout[0],
        state,
        event_ctx,
        move_list,
        ability_list,
        encounter_list,
    );

    render_evolution_list(frame, layout[1], state, event_ctx, evolution_list);
}

fn render_evolution_list(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
    evolution_list: &mut SelectList,
) {
    event_ctx.set_component_area(crate::PokeComponentId::Evolution, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("EVOLUTION")
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN))
        .border_style(focus_border(state, crate::state::FocusArea::Evolution));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items = evolution_items(state);
    if items.is_empty() {
        let message = if state.evolution_loading {
            "Evolution loading..."
        } else {
            "No evolution data."
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(TEXT_DIM))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let props = SelectListProps {
        items: &items,
        count: items.len(),
        selected: state
            .evolution_selected_index
            .min(items.len().saturating_sub(1)),
        is_focused: state.focus == crate::state::FocusArea::Evolution,
        style: evolution_list_style(),
        behavior: SelectListBehavior {
            show_scrollbar: true,
            wrap_navigation: false,
        },
        on_select: Action::EvolutionSelect,
        render_item: &|item| item.clone(),
    };
    evolution_list.render(frame, inner, props);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState, status_bar: &mut StatusBar) {
    let status = state.message.clone().unwrap_or_else(|| {
        if state.list_loading {
            "Loading pokedex...".to_string()
        } else if state.species_index_loading {
            "Loading species index...".to_string()
        } else if state.detail_loading {
            "Loading pokemon...".to_string()
        } else if state.encounter_loading {
            "Loading encounters...".to_string()
        } else if state.type_matchup_loading {
            "Loading type matchup...".to_string()
        } else if state.type_loading {
            "Loading types...".to_string()
        } else if state.region_loading {
            "Loading regions...".to_string()
        } else {
            "".to_string()
        }
    });
    let (left_hints, center_hints) = status_hints(state);
    let status_span = Span::styled(status.as_str(), Style::default().fg(ACCENT_GOLD));
    let status_items = [StatusBarItem::span(status_span)];

    let style = StatusBarStyle {
        base: BaseStyle {
            border: Some(BorderStyle {
                borders: Borders::ALL,
                style: Style::default().fg(TEXT_DIM),
                focused_style: Some(Style::default().fg(ACCENT_TEAL)),
            }),
            padding: Padding::xy(1, 0),
            bg: Some(BG_PANEL),
            fg: Some(TEXT_MAIN),
        },
        text: Style::default().fg(TEXT_DIM),
        hint_key: Style::default()
            .fg(ACCENT_TEAL)
            .add_modifier(Modifier::BOLD),
        hint_label: Style::default().fg(TEXT_DIM),
        separator: Style::default().fg(TEXT_DIM),
    };

    let props = StatusBarProps {
        left: StatusBarSection::hints(&left_hints).with_separator("  "),
        center: StatusBarSection::hints(&center_hints).with_separator("  "),
        right: StatusBarSection::items(&status_items).with_separator("  "),
        style,
        is_focused: false,
    };
    Component::<Action>::render(status_bar, frame, area, props);
}

fn status_hints(state: &AppState) -> (Vec<StatusBarHint<'static>>, Vec<StatusBarHint<'static>>) {
    if state.search.active {
        let left = vec![
            StatusBarHint::new("Enter", "Apply"),
            StatusBarHint::new("Esc", "Cancel"),
            StatusBarHint::new("Bksp", "Delete"),
        ];
        let center = vec![StatusBarHint::new("q", "Quit")];
        return (left, center);
    }

    let mut left = Vec::new();
    match state.focus {
        crate::state::FocusArea::Header => {
            left.push(StatusBarHint::new("c", "Clear"));
        }
        crate::state::FocusArea::DexList => {
            left.extend([
                StatusBarHint::new("j/k", "Move"),
                StatusBarHint::new("PgUp/PgDn", "Page"),
                StatusBarHint::new("Shift+Up/Down", "Evo"),
                StatusBarHint::new("f", "Favorite"),
                StatusBarHint::new("t", "Team"),
            ]);
        }
        crate::state::FocusArea::DetailTabs => {
            left.push(StatusBarHint::new("h/l", "Tabs"));
            match state.detail_mode {
                crate::state::DetailMode::Move
                | crate::state::DetailMode::Ability
                | crate::state::DetailMode::Encounter => {
                    left.push(StatusBarHint::new("j/k", "Select"));
                }
                crate::state::DetailMode::General | crate::state::DetailMode::Matchup => {}
            }
        }
        crate::state::FocusArea::Evolution => {
            left.push(StatusBarHint::new("j/k", "Select"));
        }
    }

    let type_label = if state.focus == crate::state::FocusArea::DetailTabs
        && state.detail_mode == crate::state::DetailMode::Encounter
    {
        "Version"
    } else {
        "Type"
    };
    let center = vec![
        StatusBarHint::new("Tab", "Focus"),
        StatusBarHint::new("/", "Search"),
        StatusBarHint::new("[ ]", type_label),
        StatusBarHint::new("r/R", "Region"),
        StatusBarHint::new("p", "Cry"),
        StatusBarHint::new("q", "Quit"),
    ];
    (left, center)
}

fn evolution_items(state: &AppState) -> Vec<Line<'static>> {
    let Some(species) = state.current_species() else {
        return Vec::new();
    };
    let Some(url) = species.evolution_chain_url.as_ref() else {
        return Vec::new();
    };
    let id = url
        .trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("unknown");
    let Some(chain) = state.evolution.get(id) else {
        return Vec::new();
    };
    chain
        .stages
        .iter()
        .enumerate()
        .map(|(idx, name)| Line::from(format!("{:02} {}", idx + 1, name)))
        .collect()
}

fn evolution_stage_count(state: &AppState) -> Option<usize> {
    let species = state.current_species()?;
    let url = species.evolution_chain_url.as_ref()?;
    let id = url
        .trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("unknown");
    let chain = state.evolution.get(id)?;
    Some(chain.stages.len())
}

fn evolution_list_style() -> SelectListStyle {
    SelectListStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(BG_PANEL),
            fg: Some(TEXT_MAIN),
        },
        selection: SelectionStyle {
            style: Some(
                Style::default()
                    .bg(BG_HIGHLIGHT)
                    .fg(TEXT_MAIN)
                    .add_modifier(Modifier::BOLD),
            ),
            marker: None,
            disabled: false,
        },
        ..SelectListStyle::default()
    }
}

fn dex_items(state: &AppState) -> Vec<Line<'static>> {
    state
        .filtered_indices
        .iter()
        .filter_map(|idx| state.pokedex.get(*idx))
        .map(|entry| {
            let fav = if state.favorites.contains(&entry.name) {
                "*"
            } else {
                " "
            };
            Line::from(format!("{} #{:03} {}", fav, entry.entry_number, entry.name))
        })
        .collect()
}

fn move_items(state: &AppState) -> Vec<Line<'static>> {
    let Some(detail) = state.current_detail() else {
        return Vec::new();
    };
    detail
        .moves
        .iter()
        .enumerate()
        .map(|(idx, name)| Line::from(format!("{:02} {name}", idx + 1)))
        .collect()
}

fn ability_items(state: &AppState) -> Vec<Line<'static>> {
    let Some(detail) = state.current_detail() else {
        return Vec::new();
    };
    detail
        .abilities
        .iter()
        .enumerate()
        .map(|(idx, name)| Line::from(format!("{:02} {name}", idx + 1)))
        .collect()
}

fn encounter_items(state: &AppState) -> Vec<Line<'static>> {
    let locations = encounter_locations(state);
    let filter = state.encounter_version_filter.as_deref();
    locations
        .iter()
        .enumerate()
        .map(|(idx, encounter)| {
            let summary = encounter_summary(encounter, filter)
                .map(|text| format!("  {text}"))
                .unwrap_or_default();
            Line::from(format!(
                "{:02} {}{}",
                idx + 1,
                format_name(&encounter.location),
                summary
            ))
        })
        .collect()
}

fn encounter_locations(state: &AppState) -> Vec<&crate::state::EncounterLocation> {
    let Some(name) = state.detail_name.as_ref() else {
        return Vec::new();
    };
    let Some(encounters) = state.encounter_cache.get(name) else {
        return Vec::new();
    };
    let filter = state.encounter_version_filter.as_deref();
    encounters
        .iter()
        .filter(|location| {
            location
                .version_details
                .iter()
                .any(|version| filter.map_or(true, |value| version.version == value))
        })
        .collect()
}

fn encounter_summary(
    encounter: &crate::state::EncounterLocation,
    filter: Option<&str>,
) -> Option<String> {
    let versions = filtered_encounter_versions(encounter, filter);
    if versions.is_empty() {
        return None;
    }
    let mut methods: HashMap<String, (u8, u8, u8)> = HashMap::new();
    for version in versions {
        for detail in &version.encounters {
            let entry = methods
                .entry(detail.method.clone())
                .or_insert((detail.min_level, detail.max_level, detail.chance));
            entry.0 = entry.0.min(detail.min_level);
            entry.1 = entry.1.max(detail.max_level);
            entry.2 = entry.2.max(detail.chance);
        }
    }
    let mut method_list: Vec<(String, u8, u8, u8)> = methods
        .into_iter()
        .map(|(method, (min, max, chance))| (method, min, max, chance))
        .collect();
    method_list.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.0.cmp(&b.0)));
    let summary = method_list
        .into_iter()
        .take(3)
        .map(|(method, min, max, _chance)| {
            let level = if min == max {
                format!("Lv{}", min)
            } else {
                format!("Lv{}-{}", min, max)
            };
            format!("{} {}", format_name(&method), level)
        })
        .collect::<Vec<_>>()
        .join(" | ");
    if summary.is_empty() {
        None
    } else {
        Some(summary)
    }
}

fn filtered_encounter_versions<'a>(
    encounter: &'a crate::state::EncounterLocation,
    filter: Option<&str>,
) -> Vec<&'a crate::state::EncounterVersion> {
    let mut versions: Vec<_> = encounter
        .version_details
        .iter()
        .filter(|version| filter.map_or(true, |value| version.version == value))
        .collect();
    versions.sort_by(|a, b| a.version.cmp(&b.version));
    versions
}

fn dex_list_style() -> SelectListStyle {
    SelectListStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: None,
            fg: Some(TEXT_MAIN),
        },
        selection: SelectionStyle {
            style: Some(
                Style::default()
                    .bg(BG_HIGHLIGHT)
                    .fg(TEXT_MAIN)
                    .add_modifier(Modifier::BOLD),
            ),
            marker: None,
            disabled: false,
        },
        ..SelectListStyle::default()
    }
}

fn detail_list_style() -> SelectListStyle {
    SelectListStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::xy(1, 0),
            bg: Some(BG_PANEL_ALT),
            fg: Some(TEXT_MAIN),
        },
        selection: SelectionStyle {
            style: Some(
                Style::default()
                    .bg(BG_HIGHLIGHT)
                    .fg(TEXT_MAIN)
                    .add_modifier(Modifier::BOLD),
            ),
            marker: None,
            disabled: false,
        },
        ..SelectListStyle::default()
    }
}

// detail blocks removed; General tab owns profile data

fn render_detail_tabs(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    event_ctx: &mut EventContext<crate::PokeComponentId>,
    move_list: &mut SelectList,
    ability_list: &mut SelectList,
    encounter_list: &mut SelectList,
) {
    event_ctx.set_component_area(crate::PokeComponentId::DetailTabs, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("DETAIL")
        .style(Style::default().bg(BG_PANEL).fg(TEXT_MAIN))
        .border_style(focus_border(state, crate::state::FocusArea::DetailTabs));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(4)])
        .split(inner);
    let tabs = Tabs::new(vec!["General", "Moves", "Abilities", "Encounters", "Matchup"])
        .select(detail_mode_index(state))
        .style(Style::default().fg(TEXT_DIM))
        .highlight_style(
            Style::default()
                .fg(ACCENT_TEAL)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, layout[0]);
    match state.detail_mode {
        crate::state::DetailMode::General => {
            let content = general_detail_text(state);
            frame.render_widget(
                Paragraph::new(content)
                    .style(Style::default().fg(TEXT_MAIN))
                    .wrap(Wrap { trim: true }),
                layout[1],
            );
        }
        crate::state::DetailMode::Move => {
            render_moves_tab(frame, layout[1], state, move_list);
        }
        crate::state::DetailMode::Ability => {
            render_abilities_tab(frame, layout[1], state, ability_list);
        }
        crate::state::DetailMode::Encounter => {
            render_encounters_tab(frame, layout[1], state, encounter_list);
        }
        crate::state::DetailMode::Matchup => {
            render_matchup_tab(frame, layout[1], state);
        }
    }
}

fn general_detail_text(state: &AppState) -> Text<'static> {
    let Some(detail) = state.current_detail() else {
        return Text::from("Select a Pokemon.");
    };
    let types = detail.types.join(" / ");
    let genus = state
        .current_species()
        .and_then(|species| species.genus.clone())
        .unwrap_or_else(|| "".to_string());
    let flavor = state
        .current_species()
        .and_then(|species| species.flavor_text.clone())
        .unwrap_or_else(|| "".to_string());
    let cry = if detail.cries_latest.is_some() || detail.cries_legacy.is_some() {
        "Cry: available"
    } else {
        "Cry: --"
    };
    let mut lines = vec![
        Line::from(Span::styled(
            format!("{}  #{:03}", detail.name.to_ascii_uppercase(), detail.id),
            Style::default()
                .fg(ACCENT_TEAL)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Type: {types}")),
        Line::from(format!(
            "Height: {}  Weight: {}",
            detail.height, detail.weight
        )),
        Line::from(format!("Genus: {genus}")),
        Line::from(cry),
    ];
    if !flavor.is_empty() {
        lines.push(Line::from(" "));
        lines.push(Line::from(flavor));
    }
    Text::from(lines)
}

fn render_moves_tab(frame: &mut Frame, area: Rect, state: &AppState, move_list: &mut SelectList) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    let list_block = Block::default()
        .borders(Borders::ALL)
        .title("MOVES")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let list_inner = list_block.inner(layout[0]);
    frame.render_widget(list_block, layout[0]);

    let items = move_items(state);
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new("No moves.")
                .style(Style::default().fg(TEXT_DIM))
                .wrap(Wrap { trim: true }),
            list_inner,
        );
    } else {
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: state.selected_move_index.min(items.len().saturating_sub(1)),
            is_focused: state.focus == crate::state::FocusArea::DetailTabs,
            style: detail_list_style(),
            behavior: SelectListBehavior {
                show_scrollbar: true,
                wrap_navigation: false,
            },
            on_select: Action::MoveSelect,
            render_item: &|item| item.clone(),
        };
        move_list.render(frame, list_inner, props);
    }

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("MOVE DETAIL")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let detail_inner = detail_block.inner(layout[1]);
    frame.render_widget(detail_block, layout[1]);
    let detail_text = move_detail_text(state);
    frame.render_widget(
        Paragraph::new(detail_text)
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true }),
        detail_inner,
    );
}

fn render_abilities_tab(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    ability_list: &mut SelectList,
) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    let list_block = Block::default()
        .borders(Borders::ALL)
        .title("ABILITIES")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let list_inner = list_block.inner(layout[0]);
    frame.render_widget(list_block, layout[0]);

    let items = ability_items(state);
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new("No abilities.")
                .style(Style::default().fg(TEXT_DIM))
                .wrap(Wrap { trim: true }),
            list_inner,
        );
    } else {
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: state
                .selected_ability_index
                .min(items.len().saturating_sub(1)),
            is_focused: state.focus == crate::state::FocusArea::DetailTabs,
            style: detail_list_style(),
            behavior: SelectListBehavior {
                show_scrollbar: false,
                wrap_navigation: false,
            },
            on_select: Action::AbilitySelect,
            render_item: &|item| item.clone(),
        };
        ability_list.render(frame, list_inner, props);
    }

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("ABILITY DETAIL")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let detail_inner = detail_block.inner(layout[1]);
    frame.render_widget(detail_block, layout[1]);
    let detail_text = ability_detail_only_text(state);
    frame.render_widget(
        Paragraph::new(detail_text)
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true }),
        detail_inner,
    );
}

fn render_encounters_tab(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    encounter_list: &mut SelectList,
) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    let filter_label = state
        .encounter_version_filter
        .as_deref()
        .map(format_name)
        .unwrap_or_else(|| "All".to_string());
    let list_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("LOCATIONS ({filter_label})"))
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let list_inner = list_block.inner(layout[0]);
    frame.render_widget(list_block, layout[0]);

    let items = encounter_items(state);
    if items.is_empty() {
        let message = if state.encounter_loading {
            "Loading encounters..."
        } else {
            "No encounter data."
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(TEXT_DIM))
                .wrap(Wrap { trim: true }),
            list_inner,
        );
    } else {
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: state
                .selected_encounter_index
                .min(items.len().saturating_sub(1)),
            is_focused: state.focus == crate::state::FocusArea::DetailTabs,
            style: detail_list_style(),
            behavior: SelectListBehavior {
                show_scrollbar: true,
                wrap_navigation: false,
            },
            on_select: Action::EncounterSelect,
            render_item: &|item| item.clone(),
        };
        encounter_list.render(frame, list_inner, props);
    }

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("ENCOUNTERS")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let detail_inner = detail_block.inner(layout[1]);
    frame.render_widget(detail_block, layout[1]);
    let detail_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(detail_inner);
    let version_text = encounter_version_text(state);
    frame.render_widget(
        Paragraph::new(version_text)
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true }),
        detail_layout[0],
    );
    let detail_text = encounter_detail_text(state);
    frame.render_widget(
        Paragraph::new(detail_text)
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true }),
        detail_layout[1],
    );
}

fn render_matchup_tab(frame: &mut Frame, area: Rect, state: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let defense_block = Block::default()
        .borders(Borders::ALL)
        .title("DEFENSE")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let offense_block = Block::default()
        .borders(Borders::ALL)
        .title("OFFENSE")
        .style(Style::default().bg(BG_PANEL_ALT).fg(TEXT_MAIN));
    let defense_inner = defense_block.inner(layout[0]);
    let offense_inner = offense_block.inner(layout[1]);
    frame.render_widget(defense_block, layout[0]);
    frame.render_widget(offense_block, layout[1]);

    let (defense_text, offense_text) = matchup_texts(state);
    frame.render_widget(
        Paragraph::new(defense_text)
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true }),
        defense_inner,
    );
    frame.render_widget(
        Paragraph::new(offense_text)
            .style(Style::default().fg(TEXT_MAIN))
            .wrap(Wrap { trim: true }),
        offense_inner,
    );
}

fn move_detail_text(state: &AppState) -> Text<'static> {
    let Some(name) = state.current_move_name() else {
        return Text::from("No move selected.");
    };
    let Some(detail) = state.move_cache.get(&name) else {
        return Text::from(format!("Loading move: {name}..."));
    };
    let power = detail
        .power
        .map(|value| value.to_string())
        .unwrap_or_else(|| "--".to_string());
    let accuracy = detail
        .accuracy
        .map(|value| value.to_string())
        .unwrap_or_else(|| "--".to_string());
    let pp = detail
        .pp
        .map(|value| value.to_string())
        .unwrap_or_else(|| "--".to_string());
    let effect = detail.effect.clone().unwrap_or_else(|| "".to_string());
    Text::from(vec![
        Line::from(Span::styled(
            detail.name.to_ascii_uppercase(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Power: {power}  Acc: {accuracy}  PP: {pp}")),
        Line::from(effect),
    ])
}

fn ability_detail_only_text(state: &AppState) -> Text<'static> {
    let Some(name) = state.current_ability_name() else {
        return Text::from("No ability selected.");
    };
    let Some(detail) = state.ability_cache.get(&name) else {
        return Text::from(format!("Loading ability: {name}..."));
    };
    let effect = detail.effect.clone().unwrap_or_else(|| "".to_string());
    Text::from(vec![
        Line::from(Span::styled(
            detail.name.to_ascii_uppercase(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(effect),
    ])
}

fn encounter_detail_text(state: &AppState) -> Text<'static> {
    let Some(name) = state.detail_name.as_ref() else {
        return Text::from("Select a Pokemon.");
    };
    let Some(_) = state.encounter_cache.get(name) else {
        if state.encounter_loading {
            return Text::from("Loading encounters...");
        }
        return Text::from("No encounter data.");
    };
    let locations = encounter_locations(state);
    if locations.is_empty() {
        return Text::from("No encounter data.");
    }
    let index = state
        .selected_encounter_index
        .min(locations.len().saturating_sub(1));
    let encounter = locations[index];
    let filter = state.encounter_version_filter.as_deref();
    let versions = filtered_encounter_versions(encounter, filter);
    let Some(active_version) = active_encounter_version(&versions, filter) else {
        return Text::from("No encounter data.");
    };
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "{} (max {}%)",
            format_name(&active_version.version),
            active_version.max_chance
        ),
        Style::default().fg(ACCENT_TEAL).add_modifier(Modifier::BOLD),
    ))];
    if active_version.encounters.is_empty() {
        lines.push(Line::from("No encounters."));
        return Text::from(lines);
    }
    for detail in &active_version.encounters {
        let level = if detail.min_level == detail.max_level {
            format!("Lv{}", detail.min_level)
        } else {
            format!("Lv{}-{}", detail.min_level, detail.max_level)
        };
        let mut line = format!(
            "{} {}% {}",
            level,
            detail.chance,
            format_name(&detail.method)
        );
        if !detail.conditions.is_empty() {
            let conditions = detail
                .conditions
                .iter()
                .map(|condition| format_name(condition))
                .collect::<Vec<_>>()
                .join(", ");
            line.push_str(&format!(" [{}]", conditions));
        }
        lines.push(Line::from(line));
    }
    Text::from(lines)
}

fn encounter_version_text(state: &AppState) -> Text<'static> {
    let Some(name) = state.detail_name.as_ref() else {
        return Text::from("Select a Pokemon.");
    };
    let Some(_) = state.encounter_cache.get(name) else {
        if state.encounter_loading {
            return Text::from("Loading encounters...");
        }
        return Text::from("No encounter data.");
    };
    let locations = encounter_locations(state);
    if locations.is_empty() {
        return Text::from("No encounter data.");
    }
    let index = state
        .selected_encounter_index
        .min(locations.len().saturating_sub(1));
    let encounter = locations[index];
    let filter = state.encounter_version_filter.as_deref();
    let versions = filtered_encounter_versions(encounter, filter);
    let active_version = active_encounter_version(&versions, filter)
        .map(|version| version.version.as_str());
    let mut lines = vec![Line::from(Span::styled(
        format_name(&encounter.location),
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(" "));
    if versions.is_empty() {
        lines.push(Line::from("No versions."));
        return Text::from(lines);
    }
    for version in versions {
        let label = format!(
            "{} ({}%)",
            format_name(&version.version),
            version.max_chance
        );
        let style = if active_version == Some(version.version.as_str()) {
            Style::default().fg(ACCENT_TEAL).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT_MAIN)
        };
        lines.push(Line::from(Span::styled(label, style)));
    }
    Text::from(lines)
}

fn active_encounter_version<'a>(
    versions: &[&'a crate::state::EncounterVersion],
    filter: Option<&str>,
) -> Option<&'a crate::state::EncounterVersion> {
    if let Some(value) = filter {
        if let Some(version) = versions.iter().find(|version| version.version == value) {
            return Some(*version);
        }
    }
    versions.first().copied()
}

fn matchup_texts(state: &AppState) -> (Text<'static>, Text<'static>) {
    let Some(detail) = state.current_detail() else {
        return (Text::from("Select a Pokemon."), Text::from("Select a Pokemon."));
    };
    if state.type_matchup_loading {
        return (
            Text::from("Loading type matchup..."),
            Text::from("Loading type matchup..."),
        );
    }
    if detail.types.is_empty() {
        return (Text::from("No type data."), Text::from("No type data."));
    }
    let defense = defense_multipliers(state, &detail.types);
    let offense = offense_multipliers(state, &detail.types);
    let Some(defense) = defense else {
        return (
            Text::from("Type data unavailable."),
            Text::from("Type data unavailable."),
        );
    };
    let Some(offense) = offense else {
        return (
            Text::from("Type data unavailable."),
            Text::from("Type data unavailable."),
        );
    };
    (
        matchup_section_text(
            &defense,
            &[
                (4.0, "Weak x4"),
                (2.0, "Weak x2"),
                (0.5, "Resist x1/2"),
                (0.25, "Resist x1/4"),
                (0.0, "Immune"),
            ],
        ),
        matchup_section_text(
            &offense,
            &[
                (2.0, "Strong x2"),
                (0.5, "Weak x1/2"),
                (0.0, "No effect"),
            ],
        ),
    )
}

fn defense_multipliers(
    state: &AppState,
    types: &[String],
) -> Option<HashMap<String, f32>> {
    if state.type_list.is_empty() {
        return None;
    }
    let mut multipliers: HashMap<String, f32> = state
        .type_list
        .iter()
        .map(|name| (name.clone(), 1.0))
        .collect();
    for type_name in types {
        let matchup = state.type_matchup_cache.get(type_name)?;
        apply_multiplier(&mut multipliers, &matchup.double_from, 2.0);
        apply_multiplier(&mut multipliers, &matchup.half_from, 0.5);
        apply_immunity(&mut multipliers, &matchup.no_from);
    }
    Some(multipliers)
}

fn offense_multipliers(
    state: &AppState,
    types: &[String],
) -> Option<HashMap<String, f32>> {
    if state.type_list.is_empty() {
        return None;
    }
    let mut multipliers: HashMap<String, f32> = state
        .type_list
        .iter()
        .map(|name| (name.clone(), 1.0))
        .collect();
    for type_name in types {
        let matchup = state.type_matchup_cache.get(type_name)?;
        let mut type_map: HashMap<String, f32> = state
            .type_list
            .iter()
            .map(|name| (name.clone(), 1.0))
            .collect();
        apply_multiplier(&mut type_map, &matchup.double_to, 2.0);
        apply_multiplier(&mut type_map, &matchup.half_to, 0.5);
        apply_immunity(&mut type_map, &matchup.no_to);
        for (name, value) in type_map {
            let entry = multipliers.entry(name).or_insert(1.0);
            if value > *entry {
                *entry = value;
            }
        }
    }
    Some(multipliers)
}

fn apply_multiplier(
    multipliers: &mut HashMap<String, f32>,
    types: &[String],
    factor: f32,
) {
    for type_name in types {
        if let Some(value) = multipliers.get_mut(type_name) {
            if *value != 0.0 {
                *value *= factor;
            }
        }
    }
}

fn apply_immunity(multipliers: &mut HashMap<String, f32>, types: &[String]) {
    for type_name in types {
        if let Some(value) = multipliers.get_mut(type_name) {
            *value = 0.0;
        }
    }
}

fn matchup_section_text(
    multipliers: &HashMap<String, f32>,
    sections: &[(f32, &'static str)],
) -> Text<'static> {
    let mut lines = Vec::new();
    for (value, label) in sections {
        let mut names: Vec<String> = multipliers
            .iter()
            .filter(|(_, multiplier)| *multiplier == value)
            .map(|(name, _)| format_name(name))
            .collect();
        if names.is_empty() {
            continue;
        }
        names.sort();
        lines.push(Line::from(Span::styled(
            *label,
            Style::default().fg(ACCENT_TEAL).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(names.join(", ")));
        lines.push(Line::from(" "));
    }
    if lines.is_empty() {
        return Text::from("No matchup data.");
    }
    Text::from(lines)
}

fn detail_mode_index(state: &AppState) -> usize {
    match state.detail_mode {
        crate::state::DetailMode::General => 0,
        crate::state::DetailMode::Move => 1,
        crate::state::DetailMode::Ability => 2,
        crate::state::DetailMode::Encounter => 3,
        crate::state::DetailMode::Matchup => 4,
    }
}

fn format_name(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => "".to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_stat(stat: &PokemonStat) -> String {
    let label = shorten_stat(&stat.name);
    let bar_len = (stat.value as usize / 10).min(20).max(1);
    let bar = "#".repeat(bar_len);
    format!("{label:>4} {value:>3} {bar}", value = stat.value)
}

fn shorten_stat(name: &str) -> String {
    match name {
        "hp" => " HP".to_string(),
        "attack" => "ATK".to_string(),
        "defense" => "DEF".to_string(),
        "special-attack" => "SAT".to_string(),
        "special-defense" => "SDF".to_string(),
        "speed" => "SPD".to_string(),
        _ => name.to_ascii_uppercase(),
    }
}

fn route_label(state: &AppState) -> (usize, usize) {
    let page_size = list_page_size(state).max(1);
    let route_index = state.selected_index / page_size + 1;
    let total_pages = (state.filtered_indices.len() + page_size - 1) / page_size;
    (route_index, total_pages.max(1))
}

fn region_counts(state: &AppState) -> (usize, usize, usize) {
    let seen = state
        .pokedex_all
        .iter()
        .filter(|entry| state.seen.contains(&entry.name))
        .count();
    let caught = state
        .pokedex_all
        .iter()
        .filter(|entry| state.favorites.contains(&entry.name))
        .count();
    let total = state.pokedex_all.len();
    (seen, caught, total)
}

fn list_page_size(state: &AppState) -> usize {
    state.terminal_size.1.saturating_sub(8) as usize
}

fn focus_border(state: &AppState, area: crate::state::FocusArea) -> Style {
    if state.focus == area {
        Style::default()
            .fg(ACCENT_TEAL)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT_DIM)
    }
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
