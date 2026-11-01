use std::{cell::RefCell, io, path::PathBuf, time::Duration};

use clap::Parser;
use ratatui::{
    backend::{Backend, TestBackend},
    Terminal,
};
use tui_dispatch::{NoEffect, StoreWithMiddleware};
use tui_dispatch_debug::{debug::DebugLayer, DebugCliArgs, DebugSession};

use space_invaders::{
    action::Action,
    input::{KeyboardMode, KeyboardSession},
    reducer::reducer,
    sprites::SpriteSizing,
    state::{AppState, MIN_HEIGHT},
    ui::{map_event_with_mode, GameUi},
    DEFAULT_SEED, TICK_MS,
};

#[derive(Parser, Debug)]
#[command(about = "A tiny, colorful Space Invaders arcade game")]
struct Args {
    #[arg(long, default_value_t = DEFAULT_SEED)]
    seed: u64,
    /// Automatically fit sprites to the terminal; compact forces 5×2 sprites.
    #[arg(long, value_enum, default_value_t = SpriteSizing::Auto)]
    sprite_size: SpriteSizing,
    /// Auto negotiates smooth held movement; legacy keeps stepped input.
    #[arg(long, value_enum, default_value_t = KeyboardMode::Auto)]
    keyboard_mode: KeyboardMode,
    /// Headless frame width (interactive play uses the terminal size).
    #[arg(long, default_value_t = 80, value_parser = clap::value_parser!(u16).range(1..=512))]
    width: u16,
    /// Headless frame height.
    #[arg(long, default_value_t = MIN_HEIGHT, value_parser = clap::value_parser!(u16).range(1..=256))]
    height: u16,
    /// Save the final gameplay state as a JSON snapshot.
    #[arg(long)]
    debug_state_out: Option<PathBuf>,
    #[command(flatten)]
    debug: DebugCliArgs,
}

struct RestoreTerminal;

impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();
    // The framework normally filters Tick out. Record it for deterministic games,
    // while preserving any explicit user filters.
    if args.debug.actions_out.is_some()
        && args.debug.actions_include.is_none()
        && args.debug.actions_exclude.is_none()
        && args.debug.actions_include_categories.is_none()
        && args.debug.actions_exclude_categories.is_none()
        && args.debug.actions_include_names.is_none()
        && args.debug.actions_exclude_names.is_none()
    {
        args.debug.actions_include = Some("*".into());
    }
    let headless = args.debug.render_once;
    let scripted = args.debug.actions_in.is_some();
    let session = DebugSession::new(args.debug.clone());
    session.save_state_schema::<AppState>()?;
    session.save_actions_schema::<Action>()?;

    if headless {
        let mut terminal = Terminal::new(TestBackend::new(args.width, args.height))?;
        run(
            &mut terminal,
            &session,
            &args,
            scripted,
            KeyboardMode::Legacy,
        )
        .await?;
    } else {
        let mut terminal = ratatui::try_init()?;
        let _restore = RestoreTerminal;
        let keyboard = KeyboardSession::start(args.keyboard_mode)?;
        run(&mut terminal, &session, &args, scripted, keyboard.mode()).await?;
    }
    Ok(())
}

async fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    session: &DebugSession,
    args: &Args,
    scripted: bool,
    keyboard_mode: KeyboardMode,
) -> Result<(), Box<dyn std::error::Error>>
where
    B::Error: Send + Sync + 'static,
{
    let mut state = session.load_state_or_else(|| Ok::<_, io::Error>(AppState::new(args.seed)))?;
    if !session.render_once() && !scripted {
        state.player.clear_controls();
    }
    let size = terminal.size()?;
    reducer(
        &mut state,
        Action::ViewportResize {
            width: size.width,
            height: size.height,
        },
    );
    let replay = session.load_replay_items::<Action>()?;
    let (middleware, recorder) = session.middleware_with_recorder();
    let store = StoreWithMiddleware::new(state, reducer, middleware);
    let mut ui = GameUi {
        sizing: args.sprite_size,
        ..GameUi::default()
    };
    let action_sender = RefCell::new(None::<tokio::sync::mpsc::UnboundedSender<Action>>);
    let output = session
        .run_effect_app(
            terminal,
            store,
            DebugLayer::simple(),
            replay,
            None,
            Some(Action::Quit),
            |runtime| {
                *action_sender.borrow_mut() = Some(runtime.action_tx());
                if !session.render_once() && !scripted {
                    runtime.subscriptions().interval(
                        "game-clock",
                        Duration::from_millis(TICK_MS),
                        || Action::Tick,
                    );
                }
            },
            |frame, area, state, ctx| {
                // DebugSession intercepts F12 before the input mapper. Queue a
                // store action when it takes focus so swallowed releases cannot
                // leave the ship moving after the overlay closes.
                if !ctx.is_focused() && state.player.held_keys != 0 {
                    if let Some(sender) = action_sender.borrow().as_ref() {
                        let _ = sender.send(Action::PlayerControlsClear);
                    }
                }
                ui.render(frame, area, state);
            },
            |event, state| map_event_with_mode(event, state, keyboard_mode),
            |action| matches!(action, Action::Quit),
            |effect: NoEffect, _ctx| match effect {},
        )
        .await?;
    output.write_render_output()?;
    session.save_actions(recorder.as_ref())?;
    if let Some(path) = &args.debug_state_out {
        std::fs::write(path, serde_json::to_string_pretty(output.state())?)?;
    }
    Ok(())
}
