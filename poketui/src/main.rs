mod action;
mod api;
mod effect;
mod reducer;
mod sprite;
mod sprite_backend;
mod state;
mod ui;

use std::io;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use rodio::{source::SineWave, OutputStream, Sink, Source};
use tui_dispatch::{
    EffectContext, EffectStoreLike, EffectStoreWithMiddleware, EventOutcome, RenderContext, TaskKey,
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
#[command(name = "poketui")]
#[command(about = "Classic Pokemon inspired TUI prototype")]
struct Args {
    #[command(flatten)]
    debug: DebugCliArgs,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let debug = DebugSession::new(args.debug);

    let state = debug
        .load_state_or_else_async(|| async { Ok::<AppState, io::Error>(AppState::new()) })
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
                    .interval("tick", Duration::from_millis(120), || Action::Tick);
            },
            |frame, area, state, render_ctx: RenderContext| {
                ui::render(frame, area, state, render_ctx);
            },
            |event, state| -> EventOutcome<Action> { ui::handle_event(event, state) },
            |action| matches!(action, Action::Quit),
            handle_effect,
        )
        .await
}

fn handle_effect(effect: Effect, ctx: &mut EffectContext<Action>) {
    match effect {
        Effect::LoadPokemon { target, name } => {
            let key = format!("pokemon_{}_{}", target.label(), name);
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_pokemon(&name).await {
                    Ok(info) => Action::PokemonDidLoad { target, info },
                    Err(error) => Action::PokemonDidError {
                        target,
                        name,
                        error,
                    },
                }
            });
        }
        Effect::LoadSprite { target, url } => {
            let key = format!("sprite_{}", target.label());
            ctx.tasks().spawn(TaskKey::new(key), async move {
                match api::fetch_bytes(&url).await {
                    Ok(bytes) => match sprite::decode_sprite(&bytes, &url) {
                        Ok(sprite) => Action::SpriteDidLoad { target, sprite },
                        Err(error) => Action::SpriteDidError { target, error },
                    },
                    Err(error) => Action::SpriteDidError { target, error },
                }
            });
        }
        Effect::PlayAttackSound => {
            play_attack_sound();
        }
        Effect::CheckSaveExists => {
            ctx.tasks().spawn(TaskKey::new("check_save"), async move {
                let path = save_file_path();
                Action::SaveExists(path.exists())
            });
        }
        Effect::SaveGame { state } => {
            ctx.tasks().spawn(TaskKey::new("save_game"), async move {
                match save_game(&state).await {
                    Ok(()) => Action::SaveComplete,
                    Err(e) => Action::SaveError(e),
                }
            });
        }
        Effect::LoadGame => {
            ctx.tasks().spawn(TaskKey::new("load_game"), async move {
                match load_game().await {
                    Ok(state) => Action::LoadComplete(Box::new(state)),
                    Err(e) => Action::LoadError(e),
                }
            });
        }
        Effect::LoadStarterPreview { name } => {
            ctx.tasks()
                .spawn(TaskKey::new("starter_preview"), async move {
                    match api::fetch_pokemon(&name).await {
                        Ok(info) => Action::StarterPreviewLoaded { info },
                        Err(error) => Action::StarterPreviewError { error },
                    }
                });
        }
        Effect::LoadStarterSprite { url } => {
            ctx.tasks()
                .spawn(TaskKey::new("starter_sprite"), async move {
                    match api::fetch_bytes(&url).await {
                        Ok(bytes) => match sprite::decode_sprite(&bytes, &url) {
                            Ok(sprite) => Action::StarterPreviewSpriteLoaded { sprite },
                            Err(error) => Action::StarterPreviewError { error },
                        },
                        Err(error) => Action::StarterPreviewError { error },
                    }
                });
        }
    }
}

fn play_attack_sound() {
    std::thread::spawn(|| {
        let Ok((stream, handle)) = OutputStream::try_default() else {
            return;
        };
        let Ok(sink) = Sink::try_new(&handle) else {
            return;
        };
        let source = SineWave::new(640.0)
            .take_duration(Duration::from_millis(140))
            .amplify(0.18);
        sink.append(source);
        sink.sleep_until_end();
        drop(stream);
    });
}

fn save_file_path() -> PathBuf {
    let base = dirs_next::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("poketui").join("save.json")
}

async fn save_game(state: &AppState) -> Result<(), String> {
    let path = save_file_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create save directory: {}", e))?;
    }
    let json =
        serde_json::to_string_pretty(state).map_err(|e| format!("Failed to serialize: {}", e))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("Failed to write save file: {}", e))?;
    Ok(())
}

async fn load_game() -> Result<AppState, String> {
    let path = save_file_path();
    let json = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                return Err("Save file not found.".to_string());
            }
            return Err(format!("Failed to read save file: {}", e));
        }
    };
    let state: AppState =
        serde_json::from_str(&json).map_err(|e| format!("Save file corrupted: {}", e))?;
    Ok(state)
}
