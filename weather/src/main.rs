//! Weather TUI - tui-dispatch example

use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend, layout::Rect};
use tui_dispatch::{
    EffectContext, EffectStoreLike, EffectStoreWithMiddleware, EventBus, EventContext, EventKind,
    EventRoutingState, HandlerResponse, Keybindings, RenderContext, TaskKey,
};
use tui_dispatch_components::centered_rect;
use tui_dispatch_debug::debug::DebugLayer;
use tui_dispatch_debug::{
    DebugCliArgs, DebugRunOutput, DebugSession, DebugSessionError, ReplayItem,
};
use weather::action::Action;
use weather::api;
use weather::api::GeocodingError;
use weather::components::{
    Component, SearchOverlay, SearchOverlayProps, WeatherDisplay, WeatherDisplayProps,
};
use weather::effect::Effect;
use weather::reducer::reducer;
use weather::state::{AppState, LOADING_ANIM_TICK_MS};

/// Weather TUI - tui-dispatch framework example
#[derive(Parser, Debug)]
#[command(name = "weather")]
#[command(about = "A weather TUI demonstrating tui-dispatch patterns")]
struct Args {
    /// City name to look up (uses Open-Meteo geocoding)
    #[arg(long, short, default_value = "Kyiv")]
    city: String,

    /// Refresh interval in seconds (minimum 1)
    #[arg(long, short, default_value = "30", value_parser = clap::value_parser!(u64).range(1..))]
    refresh_interval: u64,

    #[command(flatten)]
    debug: DebugCliArgs,
}

#[derive(tui_dispatch::ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum WeatherComponentId {
    Display,
    Search,
}

#[derive(tui_dispatch::BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum WeatherContext {
    Main,
    Search,
}

impl EventRoutingState<WeatherComponentId, WeatherContext> for AppState {
    fn focused(&self) -> Option<WeatherComponentId> {
        if self.search_mode {
            Some(WeatherComponentId::Search)
        } else {
            Some(WeatherComponentId::Display)
        }
    }

    fn modal(&self) -> Option<WeatherComponentId> {
        if self.search_mode {
            Some(WeatherComponentId::Search)
        } else {
            None
        }
    }

    fn binding_context(&self, id: WeatherComponentId) -> WeatherContext {
        match id {
            WeatherComponentId::Display => WeatherContext::Main,
            WeatherComponentId::Search => WeatherContext::Search,
        }
    }

    fn default_context(&self) -> WeatherContext {
        WeatherContext::Main
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let Args {
        city,
        refresh_interval,
        debug: debug_args,
    } = Args::parse();

    let debug = DebugSession::new(debug_args);

    // Export JSON schemas if requested
    debug.save_state_schema::<AppState>().map_err(debug_error)?;
    debug.save_actions_schema::<Action>().map_err(debug_error)?;

    let state = debug
        .load_state_or_else_async(move || async move {
            let location = match api::geocode_city(&city).await {
                Ok(loc) => loc,
                Err(e) => {
                    match e {
                        GeocodingError::NotFound(city) => {
                            eprintln!(
                                "Error: City '{}' not found. Please check the spelling.",
                                city
                            );
                            eprintln!("Examples: 'London', 'Tokyo', 'New York'");
                        }
                        GeocodingError::Request(e) => {
                            eprintln!("Error: Could not connect to geocoding service.");
                            eprintln!("Details: {}", e);
                        }
                    }
                    std::process::exit(1);
                }
            };

            Ok::<AppState, io::Error>(AppState::new(location))
        })
        .await
        .map_err(debug_error)?;

    let replay_actions = debug.load_replay_items().map_err(debug_error)?;

    let (middleware, action_recorder) = debug.middleware_with_recorder();
    let store = EffectStoreWithMiddleware::new(state, reducer, middleware);

    // ===== Terminal setup =====
    let use_alt_screen = debug.use_alt_screen();
    let mut stdout = io::stdout();
    if use_alt_screen {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(
        &mut terminal,
        &debug,
        store,
        refresh_interval,
        replay_actions,
    )
    .await;

    // ===== Cleanup =====
    if use_alt_screen {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
    }
    if use_alt_screen {
        terminal.show_cursor()?;
    }

    let run_output = result?;
    run_output.write_render_output()?;
    debug
        .save_actions(action_recorder.as_ref())
        .map_err(debug_error)?;

    Ok(())
}

struct WeatherUi {
    display: WeatherDisplay,
    search: SearchOverlay,
}

impl WeatherUi {
    fn new() -> Self {
        Self {
            display: WeatherDisplay,
            search: SearchOverlay::new(),
        }
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        render_ctx: RenderContext,
        event_ctx: &mut EventContext<WeatherComponentId>,
    ) {
        event_ctx.set_component_area(WeatherComponentId::Display, area);

        let props = WeatherDisplayProps {
            state,
            is_focused: render_ctx.is_focused() && !state.search_mode,
        };
        self.display.render(frame, area, props);

        self.search.set_open(state.search_mode);
        if state.search_mode {
            let modal_area = centered_rect(60, 12, area);
            event_ctx.set_component_area(WeatherComponentId::Search, modal_area);
            let props = SearchOverlayProps {
                query: &state.search_query,
                results: &state.search_results,
                selected: state.search_selected,
                is_focused: render_ctx.is_focused(),
                error: state.search_error.as_deref(),
                on_query_change: Action::SearchQueryChange,
                on_query_submit: Action::SearchQuerySubmit,
                on_select: Action::SearchSelect,
            };
            self.search.render(frame, area, props);
        } else {
            event_ctx
                .component_areas
                .remove(&WeatherComponentId::Search);
        }
    }

    fn handle_display_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> HandlerResponse<Action> {
        let props = WeatherDisplayProps {
            state,
            is_focused: true,
        };
        let actions: Vec<_> = self
            .display
            .handle_event(event, props)
            .into_iter()
            .collect();
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

    fn handle_search_event(
        &mut self,
        event: &EventKind,
        state: &AppState,
    ) -> HandlerResponse<Action> {
        self.search.set_open(state.search_mode);
        let props = SearchOverlayProps {
            query: &state.search_query,
            results: &state.search_results,
            selected: state.search_selected,
            is_focused: true,
            error: state.search_error.as_deref(),
            on_query_change: Action::SearchQueryChange,
            on_query_submit: Action::SearchQuerySubmit,
            on_select: Action::SearchSelect,
        };
        let actions: Vec<_> = self.search.handle_event(event, props).into_iter().collect();
        HandlerResponse {
            actions,
            consumed: true,
            needs_render: false,
        }
    }
}

fn debug_error(error: DebugSessionError) -> io::Error {
    io::Error::other(format!("debug session error: {error}"))
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    debug: &DebugSession,
    store: impl EffectStoreLike<AppState, Action, Effect>,
    refresh_interval: u64,
    replay_actions: Vec<ReplayItem<Action>>,
) -> io::Result<DebugRunOutput<AppState>> {
    let ui = Rc::new(RefCell::new(WeatherUi::new()));
    let mut bus: EventBus<AppState, Action, WeatherComponentId, WeatherContext> = EventBus::new();
    let keybindings: Keybindings<WeatherContext> = Keybindings::new();

    let ui_display = Rc::clone(&ui);
    bus.register(WeatherComponentId::Display, move |event, state| {
        ui_display
            .borrow_mut()
            .handle_display_event(&event.kind, state)
    });

    let ui_search = Rc::clone(&ui);
    bus.register(WeatherComponentId::Search, move |event, state| {
        ui_search
            .borrow_mut()
            .handle_search_event(&event.kind, state)
    });

    // Re-render on terminal resize (no action needed, just redraw)
    bus.register_global(|event, _state| match event.kind {
        EventKind::Resize(_, _) => HandlerResponse::ignored().with_render(),
        _ => HandlerResponse::ignored(),
    });

    debug
        .run_effect_app_with_bus(
            terminal,
            store,
            DebugLayer::simple(),
            replay_actions,
            Some(Action::WeatherFetch),
            Some(Action::Quit),
            |runtime| {
                if debug.render_once() {
                    return;
                }

                runtime.subscriptions().interval(
                    "tick",
                    Duration::from_millis(LOADING_ANIM_TICK_MS),
                    || Action::Tick,
                );

                runtime.subscriptions().interval(
                    "refresh",
                    Duration::from_secs(refresh_interval),
                    || Action::WeatherFetch,
                );
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

/// Handle effects by spawning tasks
fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::FetchWeather { lat, lon } => {
            ctx.tasks().spawn("weather", async move {
                match api::fetch_weather_data(lat, lon).await {
                    Ok(data) => Action::WeatherDidLoad(data),
                    Err(e) => Action::WeatherDidError(e),
                }
            });
        }
        Effect::SearchCities { query } => {
            let query = query.trim().to_string();
            if query.is_empty() {
                ctx.tasks().cancel(&TaskKey::new("city_search"));
                return;
            }
            ctx.tasks()
                .debounce("city_search", Duration::from_millis(300), async move {
                    match api::search_cities(&query).await {
                        Ok(results) => Action::SearchDidLoad(results),
                        Err(e) => Action::SearchDidError(e.to_string()),
                    }
                });
        }
    }
}
