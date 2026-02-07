mod action;
mod effect;
mod llm;
mod persist;
mod reducer;
mod rules;
mod scenario;
mod state;
mod ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use tui_dispatch::{
    EffectContext, EffectStoreLike, EffectStoreWithMiddleware, EventOutcome, RenderContext, TaskKey,
};
use tui_dispatch_debug::debug::DebugLayer;
use tui_dispatch_debug::{DebugCliArgs, DebugRunOutput, DebugSession, DebugSessionError, ReplayItem};

use crate::action::Action;
use crate::effect::Effect;
use crate::llm::{client_for, Provider};
use crate::reducer::reducer;
use crate::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "dndtui")]
#[command(about = "Map-based DnD TUI with restricted LLM")]
struct Args {
    #[command(flatten)]
    debug: DebugCliArgs,
    #[arg(long, default_value = "assets/scenarios/starter")]
    scenario: String,
    #[arg(long, value_enum, default_value = "openai")]
    provider: Provider,
    #[arg(long, default_value = "gpt-4o-mini")]
    model: String,
    #[arg(long)]
    save_dir: Option<String>,
}

#[derive(Clone, Debug)]
struct RuntimeConfig {
    scenario: String,
    provider: Provider,
    model: String,
    save_path: String,
    ollama_base_url: Option<String>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let debug = DebugSession::new(args.debug);
    debug.save_state_schema::<AppState>().map_err(debug_error)?;
    debug.save_actions_schema::<Action>().map_err(debug_error)?;

    let save_path = save_file_path(args.save_dir.as_deref());
    let config = RuntimeConfig {
        scenario: args.scenario.clone(),
        provider: args.provider.clone(),
        model: args.model.clone(),
        save_path: save_path.clone(),
        ollama_base_url: std::env::var("OLLAMA_BASE_URL").ok(),
    };

    let mut state = debug
        .load_state_or_else_async(|| {
            let config = config.clone();
            async move {
                Ok::<AppState, io::Error>(AppState::new(
                    config.scenario.clone(),
                    config.save_path.clone(),
                    config.provider.clone(),
                    config.model.clone(),
                ))
            }
        })
        .await
        .map_err(debug_error)?;

    state.scenario_dir = config.scenario.clone();
    state.save_path = config.save_path.clone();
    state.provider = config.provider.clone();
    state.model = config.model.clone();

    let replay_actions = debug.load_replay_items().map_err(debug_error)?;
    let (middleware, recorder) = debug.middleware_with_recorder();
    let store = EffectStoreWithMiddleware::new(state, reducer, middleware);

    let use_alt_screen = debug.use_alt_screen();
    let mut stdout = io::stdout();
    if use_alt_screen {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    }
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &debug, store, replay_actions, config.clone()).await;

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
    config: RuntimeConfig,
) -> io::Result<DebugRunOutput<AppState>> {
    let config = Arc::new(config);
    debug
        .run_effect_app(
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
                    .interval("tick", Duration::from_millis(200), || Action::Tick);
            },
            |frame, area, state, render_ctx: RenderContext| {
                ui::render(frame, area, state, render_ctx);
            },
            |event, state| -> EventOutcome<Action> { ui::handle_event(event, state) },
            |action| matches!(action, Action::Quit),
            move |effect, ctx| handle_effect(effect, ctx, config.clone()),
        )
        .await
}

fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>, config: Arc<RuntimeConfig>) {
    match effect {
        Effect::CallLlmDialogue { npc_id, request } => {
            let provider = config.provider.clone();
            let model = config.model.clone();
            let base_url = config.ollama_base_url.clone();
            ctx.tasks().spawn(TaskKey::new("llm_dialogue"), async move {
                let api_key = std::env::var("OPENAI_API_KEY").ok();
                let client = match client_for(provider, model, api_key, base_url) {
                    Ok(client) => client,
                    Err(err) => return Action::LlmError(err.to_string()),
                };
                let mut sink = |_| {};
                match client.stream_chat(&request, &mut sink).await {
                    Ok(raw_json) => match crate::llm::schema::parse_dialogue_response(&raw_json) {
                        Ok(parsed) => Action::DialogueResponse {
                            npc_id,
                            line: parsed.npc_line,
                        },
                        Err(err) => Action::LlmError(err),
                    },
                    Err(err) => Action::LlmError(err.to_string()),
                }
            });
        }
        Effect::CallLlmInterpretAction { request } => {
            let provider = config.provider.clone();
            let model = config.model.clone();
            let base_url = config.ollama_base_url.clone();
            ctx.tasks()
                .spawn(TaskKey::new("llm_action"), async move {
                    let api_key = std::env::var("OPENAI_API_KEY").ok();
                    let client = match client_for(provider, model, api_key, base_url) {
                        Ok(client) => client,
                        Err(err) => return Action::LlmError(err.to_string()),
                    };
                    let mut sink = |_| {};
                    match client.stream_chat(&request, &mut sink).await {
                        Ok(raw_json) => {
                            match crate::llm::schema::parse_action_interpretation(&raw_json) {
                                Ok(parsed) => Action::CustomActionInterpreted(parsed),
                                Err(err) => Action::LlmError(err),
                            }
                        }
                        Err(err) => Action::LlmError(err.to_string()),
                    }
                });
        }
        Effect::SaveGame { state, since } => {
            ctx.tasks().spawn(TaskKey::new("save"), async move {
                match persist::save_game(&state, since).await {
                    Ok(()) => Action::SaveComplete,
                    Err(e) => Action::SaveError(e),
                }
            });
        }
        Effect::LoadGame { path } => {
            ctx.tasks().spawn(TaskKey::new("load"), async move {
                match persist::load_game(&path).await {
                    Ok(state) => Action::LoadComplete(Box::new(state)),
                    Err(e) => Action::LoadError(e),
                }
            });
        }
        Effect::LoadScenario { path } => {
            ctx.tasks().spawn(TaskKey::new("scenario"), async move {
                match scenario::load_scenario(std::path::Path::new(&path)).await {
                    Ok(scenario) => Action::ScenarioLoaded { scenario },
                    Err(error) => Action::ScenarioLoadError { error },
                }
            });
        }
    }
}

fn save_file_path(save_dir: Option<&str>) -> String {
    let base = save_dir
        .map(std::path::PathBuf::from)
        .or_else(|| dirs_next::data_local_dir().map(|dir| dir.join("dndtui")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("save.json")
        .to_string_lossy()
        .to_string()
}
