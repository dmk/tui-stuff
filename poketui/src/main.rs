mod action;
mod api;
mod effect;
mod reducer;
mod sprite;
mod sprite_backend;
mod state;
mod ui;

use std::io;
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
use tui_dispatch_debug::{DebugCliArgs, DebugRunOutput, DebugSession, DebugSessionError, ReplayItem};

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
                    Err(error) => Action::PokemonDidError { target, name, error },
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
