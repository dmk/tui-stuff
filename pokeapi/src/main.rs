mod action;
mod api;
mod audio;
mod effect;
mod reducer;
mod sprite;
mod sprite_backend;
mod state;
mod ui;

use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use tui_dispatch::{
    EffectContext, EffectStoreLike, EffectStoreWithMiddleware, EventBus, EventKind,
    EventRoutingState, HandlerResponse, Keybindings, TaskKey,
};
use tui_dispatch_debug::debug::DebugLayer;
use tui_dispatch_debug::{
    DebugCliArgs, DebugRunOutput, DebugSession, DebugSessionError, ReplayItem,
};

use crate::action::Action;
use crate::effect::Effect;
use crate::reducer::reducer;
use crate::sprite_backend::SpriteBackend;
use crate::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "pokeapi-tui")]
#[command(about = "PokeAPI TUI with retro styling")]
struct Args {
    #[command(flatten)]
    debug: DebugCliArgs,
}

#[derive(tui_dispatch::ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum PokeComponentId {
    Header,
    DexList,
    DetailTabs,
    Evolution,
    Search,
}

#[derive(tui_dispatch::BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum PokeContext {
    Header,
    DexList,
    DetailTabs,
    Evolution,
    Search,
}

impl EventRoutingState<PokeComponentId, PokeContext> for AppState {
    fn focused(&self) -> Option<PokeComponentId> {
        if self.search.active {
            return Some(PokeComponentId::Search);
        }
        match self.focus {
            crate::state::FocusArea::Header => Some(PokeComponentId::Header),
            crate::state::FocusArea::DexList => Some(PokeComponentId::DexList),
            crate::state::FocusArea::DetailTabs => Some(PokeComponentId::DetailTabs),
            crate::state::FocusArea::Evolution => Some(PokeComponentId::Evolution),
        }
    }

    fn modal(&self) -> Option<PokeComponentId> {
        if self.search.active {
            Some(PokeComponentId::Search)
        } else {
            None
        }
    }

    fn binding_context(&self, _id: PokeComponentId) -> PokeContext {
        match _id {
            PokeComponentId::Header => PokeContext::Header,
            PokeComponentId::DexList => PokeContext::DexList,
            PokeComponentId::DetailTabs => PokeContext::DetailTabs,
            PokeComponentId::Evolution => PokeContext::Evolution,
            PokeComponentId::Search => PokeContext::Search,
        }
    }

    fn default_context(&self) -> PokeContext {
        PokeContext::DexList
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let debug = DebugSession::new(args.debug);

    let state = debug
        .load_state_or_else_async(|| async { Ok::<AppState, io::Error>(AppState::default()) })
        .await
        .map_err(debug_error)?;
    let replay_actions = debug.load_replay_items().map_err(debug_error)?;
    let (middleware, recorder) = debug.middleware_with_recorder();
    let store = EffectStoreWithMiddleware::new(state, reducer, middleware);

    let use_alt_screen = debug.use_alt_screen();
    let mut stdout = io::stdout();
    if use_alt_screen {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    }
    let backend = SpriteBackend::new(stdout, sprite_backend::sprite_registry());
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &debug, store, replay_actions).await;

    if use_alt_screen {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
    }

    let run_output = result?;
    run_output.write_render_output()?;
    debug.save_actions(recorder.as_ref()).map_err(debug_error)?;
    Ok(())
}

fn debug_error(error: DebugSessionError) -> io::Error {
    io::Error::other(format!("debug session error: {error}"))
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    debug: &DebugSession,
    store: impl EffectStoreLike<AppState, Action, Effect>,
    replay_actions: Vec<ReplayItem<Action>>,
) -> io::Result<DebugRunOutput<AppState>> {
    let ui = Rc::new(RefCell::new(ui::PokeUi::new()));
    let mut bus: EventBus<AppState, Action, PokeComponentId, PokeContext> = EventBus::new();
    let keybindings: Keybindings<PokeContext> = Keybindings::new();

    let ui_header = Rc::clone(&ui);
    bus.register(PokeComponentId::Header, move |event, state| {
        ui_header
            .borrow_mut()
            .handle_header_event(&event.kind, state)
    });

    let ui_list = Rc::clone(&ui);
    bus.register(PokeComponentId::DexList, move |event, state| {
        ui_list
            .borrow_mut()
            .handle_list_event(&event.kind, state)
    });

    let ui_tabs = Rc::clone(&ui);
    bus.register(PokeComponentId::DetailTabs, move |event, state| {
        ui_tabs
            .borrow_mut()
            .handle_detail_tabs_event(&event.kind, state)
    });

    let ui_evo = Rc::clone(&ui);
    bus.register(PokeComponentId::Evolution, move |event, state| {
        ui_evo
            .borrow_mut()
            .handle_evolution_event(&event.kind, state)
    });

    let ui_search = Rc::clone(&ui);
    bus.register(PokeComponentId::Search, move |event, state| {
        ui_search
            .borrow_mut()
            .handle_search_event(&event.kind, state)
    });

    bus.register_global(|event, state| match event.kind {
        EventKind::Resize(width, height) => {
            HandlerResponse::action(Action::UiTerminalResize(width, height)).with_render()
        }
        EventKind::Key(key) => match key.code {
            crossterm::event::KeyCode::Char('q') => HandlerResponse::action(Action::Quit),
            crossterm::event::KeyCode::Tab => HandlerResponse::action(Action::FocusNext),
            crossterm::event::KeyCode::BackTab => HandlerResponse::action(Action::FocusPrev),
            crossterm::event::KeyCode::Char('/') if !state.search.active => {
                HandlerResponse::action(Action::SearchStart)
            }
            crossterm::event::KeyCode::Char('[') if !state.search.active => {
                if state.focus == crate::state::FocusArea::DetailTabs
                    && state.detail_mode == crate::state::DetailMode::Encounter
                {
                    HandlerResponse::action(Action::EncounterFilterPrev)
                } else {
                    HandlerResponse::action(Action::TypeFilterPrev)
                }
            }
            crossterm::event::KeyCode::Char(']') if !state.search.active => {
                if state.focus == crate::state::FocusArea::DetailTabs
                    && state.detail_mode == crate::state::DetailMode::Encounter
                {
                    HandlerResponse::action(Action::EncounterFilterNext)
                } else {
                    HandlerResponse::action(Action::TypeFilterNext)
                }
            }
            crossterm::event::KeyCode::Char('r') if !state.search.active => {
                HandlerResponse::action(Action::RegionNext)
            }
            crossterm::event::KeyCode::Char('R') if !state.search.active => {
                HandlerResponse::action(Action::RegionPrev)
            }
            crossterm::event::KeyCode::Char('p') if !state.search.active => {
                HandlerResponse::action(Action::PlayCry)
            }
            _ => HandlerResponse::ignored(),
        },
        _ => HandlerResponse::ignored(),
    });

    debug
        .run_effect_app_with_bus(
            terminal,
            store,
            DebugLayer::simple(),
            replay_actions,
            Some(Action::Init),
            Some(Action::Quit),
            |runtime| {
                if debug.render_once() {
                    return;
                }
                runtime
                    .subscriptions()
                    .interval("tick", Duration::from_millis(90), || Action::Tick);
            },
            &mut bus,
            &keybindings,
            |frame, area, state, render_ctx, event_ctx| {
                ui.borrow_mut()
                    .render(frame, area, state, render_ctx, event_ctx);
            },
            |action| matches!(action, Action::Quit),
            handle_effect,
        )
        .await
}

fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::LoadPokedex { name } => {
            ctx.tasks().spawn(TaskKey::new("pokedex"), async move {
                match api::fetch_pokedex(&name).await {
                    Ok(entries) => Action::PokedexDidLoad(entries),
                    Err(err) => Action::PokedexDidError(err),
                }
            });
        }
        Effect::LoadRegions => {
            ctx.tasks().spawn(TaskKey::new("regions"), async {
                match api::fetch_regions().await {
                    Ok(regions) => Action::RegionsDidLoad(regions),
                    Err(err) => Action::RegionsDidError(err),
                }
            });
        }
        Effect::LoadSpeciesIndex { names } => {
            ctx.tasks().spawn(TaskKey::new("species_index"), async move {
                match api::fetch_species_index(&names).await {
                    Ok(species) => Action::SpeciesIndexDidLoad(species),
                    Err(error) => Action::SpeciesIndexDidError(error),
                }
            });
        }
        Effect::LoadTypes => {
            ctx.tasks().spawn(TaskKey::new("types"), async {
                match api::fetch_type_list().await {
                    Ok(types) => Action::TypesDidLoad(types),
                    Err(err) => Action::TypesDidError(err),
                }
            });
        }
        Effect::LoadTypeDetail { name } => {
            let key = format!("type_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_type_detail(&name).await {
                    Ok(pokemon) => Action::TypeFilterDidLoad { name, pokemon },
                    Err(error) => Action::TypeFilterDidError { name, error },
                }
            });
        }
        Effect::LoadPokemonDetail { name } => {
            let key = format!("pokemon_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_pokemon_detail(&name).await {
                    Ok(detail) => Action::PokemonDidLoad(detail),
                    Err(error) => Action::PokemonDidError { name, error },
                }
            });
        }
        Effect::LoadPokemonSpecies { name } => {
            let key = format!("species_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_pokemon_species(&name).await {
                    Ok(species) => Action::PokemonSpeciesDidLoad(species),
                    Err(error) => Action::PokemonSpeciesDidError { name, error },
                }
            });
        }
        Effect::LoadEncounters { name } => {
            let key = format!("encounters_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_pokemon_encounters(&name).await {
                    Ok(encounters) => Action::EncounterDidLoad { name, encounters },
                    Err(error) => Action::EncounterDidError { name, error },
                }
            });
        }
        Effect::LoadTypeMatchup { name } => {
            let key = format!("type_matchup_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_type_matchup(&name).await {
                    Ok(matchup) => Action::TypeMatchupDidLoad { name, matchup },
                    Err(error) => Action::TypeMatchupDidError { name, error },
                }
            });
        }
        Effect::LoadEvolutionChain { id, url } => {
            let key = format!("evo_{id}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_evolution_chain(&id, &url).await {
                    Ok(chain) => Action::EvolutionDidLoad { id, chain },
                    Err(error) => Action::EvolutionDidError { id, error },
                }
            });
        }
        Effect::LoadSprite { name, url } => {
            let key = format!("sprite_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_bytes(&url).await {
                    Ok(bytes) => match sprite::decode_sprite(&bytes, &url) {
                        Ok(sprite) => Action::SpriteDidLoad { name, sprite },
                        Err(error) => Action::SpriteDidError { name, error },
                    },
                    Err(error) => Action::SpriteDidError { name, error },
                }
            });
        }
        Effect::PlayCry { name, url } => {
            ctx.tasks().spawn(TaskKey::new("cry"), async move {
                match api::fetch_bytes(&url).await {
                    Ok(bytes) => {
                        match tokio::task::spawn_blocking(move || audio::play_ogg(bytes)).await {
                            Ok(Ok(())) => Action::Tick,
                            Ok(Err(error)) => Action::CryDidError(error),
                            Err(error) => Action::CryDidError(error.to_string()),
                        }
                    }
                    Err(error) => Action::CryDidError(format!("{name}: {error}")),
                }
            });
        }
        Effect::LoadMoveDetail { name } => {
            let key = format!("move_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_move_detail(&name).await {
                    Ok(detail) => Action::MoveDetailDidLoad(detail),
                    Err(error) => Action::MoveDetailDidError { name, error },
                }
            });
        }
        Effect::LoadAbilityDetail { name } => {
            let key = format!("ability_{name}");
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_ability_detail(&name).await {
                    Ok(detail) => Action::AbilityDetailDidLoad(detail),
                    Err(error) => Action::AbilityDetailDidError { name, error },
                }
            });
        }
    }
}
