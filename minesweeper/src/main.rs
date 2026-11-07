use std::{
    cell::{Cell, RefCell},
    io,
    path::PathBuf,
    time::Duration,
};

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

use minesweeper::{
    action::Action,
    custom_board, input,
    middleware::{MinesweeperMiddleware, REVEAL_TICK_MS},
    reducer::reducer,
    state::{AppState, Difficulty},
    ui,
};

#[derive(Parser, Debug)]
#[command(about = "Minesweeper — safe openings and custom boards")]
struct Args {
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
    difficulty: u8,
    /// Headless frame width. Interactive mode uses the terminal dimensions.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=512))]
    width: u16,
    #[arg(long, default_value_t = 40, value_parser = clap::value_parser!(u16).range(1..=256))]
    height: u16,
    /// Save the final board.
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
    // Include reveal waves in action recordings.
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
    let state = session.load_state_or_else(|| {
        Ok::<_, io::Error>(AppState::new(
            [
                Difficulty::Beginner,
                Difficulty::Intermediate,
                Difficulty::Expert,
            ][args.difficulty as usize - 1],
        ))
    })?;
    state
        .validate()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
    let size = terminal.size()?;
    let area = Cell::new(ratatui::layout::Rect::new(0, 0, size.width, size.height));
    let custom = RefCell::new(custom_board::Ui::default());
    let replay = session.load_replay_items::<Action>()?;
    let (mut middleware, recorder) = session.middleware_with_recorder();
    middleware.add(MinesweeperMiddleware::default());
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
                        "minesweeper-reveal",
                        Duration::from_millis(REVEAL_TICK_MS),
                        || Action::RevealWave,
                    );
                }
            },
            |frame, bounds, state, _ctx| {
                ui::render_in(frame, bounds, state);
                custom.borrow_mut().render(frame, bounds, state);
            },
            |event, state| {
                if let tui_dispatch::EventKind::Resize(width, height) = event {
                    area.set(ratatui::layout::Rect::new(0, 0, *width, *height));
                }
                input::map_event(event, state, area.get(), &mut custom.borrow_mut())
            },
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
