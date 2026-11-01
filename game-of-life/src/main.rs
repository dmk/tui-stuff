use std::{io, path::PathBuf, time::Duration};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use ratatui::{
    backend::{Backend, TestBackend},
    Terminal,
};
use tui_dispatch::{NoEffect, StoreWithMiddleware};
use tui_dispatch_debug::{debug::DebugLayer, DebugCliArgs, DebugSession};

use game_of_life::{action::Action, reducer::reducer, state::AppState, ui, DEFAULT_SEED, TICK_MS};

#[derive(Parser, Debug)]
#[command(about = "Life — a colorful cellular observatory")]
struct Args {
    #[arg(long, default_value_t = DEFAULT_SEED)]
    seed: u64,
    /// Headless frame width. Interactive mode uses the terminal dimensions.
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u16).range(1..=512))]
    width: u16,
    #[arg(long, default_value_t = 42, value_parser = clap::value_parser!(u16).range(1..=256))]
    height: u16,
    /// Save the final state, including cell ages, trails, and the random seed.
    #[arg(long)]
    debug_state_out: Option<PathBuf>,
    #[command(flatten)]
    debug: DebugCliArgs,
}

struct RestoreTerminal;

impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        ratatui::restore();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();
    // Include the subscription clock in recordings so they replay exactly.
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
    let session = DebugSession::new(args.debug.clone());
    session.save_state_schema::<AppState>()?;
    session.save_actions_schema::<Action>()?;
    if session.render_once() {
        let mut terminal = Terminal::new(TestBackend::new(args.width, args.height))?;
        run(&mut terminal, &session, &args).await?;
    } else {
        let mut terminal = ratatui::try_init()?;
        let _restore = RestoreTerminal;
        execute!(io::stdout(), EnableMouseCapture)?;
        run(&mut terminal, &session, &args).await?;
    }
    Ok(())
}

async fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    session: &DebugSession,
    args: &Args,
) -> Result<(), Box<dyn std::error::Error>>
where
    B::Error: Send + Sync + 'static,
{
    let mut state = session.load_state_or_else(|| Ok::<_, io::Error>(AppState::new(args.seed)))?;
    state
        .validate()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
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
    let output = session
        .run_effect_app(
            terminal,
            store,
            DebugLayer::simple(),
            replay,
            None,
            Some(Action::Quit),
            |runtime| {
                if !session.render_once() && args.debug.actions_in.is_none() {
                    runtime.subscriptions().interval(
                        "life-clock",
                        Duration::from_millis(TICK_MS),
                        || Action::Tick,
                    );
                }
            },
            |frame, area, state, _ctx| ui::render(frame, area, state),
            ui::map_event,
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
