mod action;
mod danger;
mod effect;
mod lighting;
mod procgen;
mod reducer;
mod state;
mod ui;

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tui_dispatch::EffectStore;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::{AppState, Direction, GameMode};

#[derive(Parser, Debug)]
#[command(name = "lightline")]
#[command(about = "Prototype scaffold for Lightline")]
struct Args {
    #[arg(long, default_value_t = 0xC0FF_EE_u64)]
    seed: u64,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, args.seed);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, seed: u64) -> io::Result<()> {
    let mut store = EffectStore::new(AppState::new(seed), reducer::reducer);
    dispatch_action(&mut store, Action::Init);

    loop {
        terminal.draw(|frame| ui::render(frame, frame.area(), store.state()))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if handle_key(key.code, key.modifiers, &mut store) {
                        break;
                    }
                }
                _ => {}
            }
        }

        dispatch_action(&mut store, Action::Tick);
    }

    Ok(())
}

fn handle_key(
    code: KeyCode,
    modifiers: KeyModifiers,
    store: &mut EffectStore<AppState, Action, Effect>,
) -> bool {
    let mode = store.state().mode;
    let collect = modifiers.contains(KeyModifiers::SHIFT);

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => true,
        KeyCode::Esc => {
            if mode == GameMode::Pause {
                dispatch_action(store, Action::PauseClose);
            } else {
                dispatch_action(store, Action::PauseOpen);
            }
            false
        }
        KeyCode::Char('r') | KeyCode::Char('R') if mode == GameMode::GameOver => {
            dispatch_action(store, Action::Init);
            false
        }
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') if mode == GameMode::Exploration => {
            dispatch_action(store, Action::Move(Direction::Up, collect));
            false
        }
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S')
            if mode == GameMode::Exploration =>
        {
            dispatch_action(store, Action::Move(Direction::Down, collect));
            false
        }
        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A')
            if mode == GameMode::Exploration =>
        {
            dispatch_action(store, Action::Move(Direction::Left, collect));
            false
        }
        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D')
            if mode == GameMode::Exploration =>
        {
            dispatch_action(store, Action::Move(Direction::Right, collect));
            false
        }
        KeyCode::Char('e') | KeyCode::Char('E') if mode == GameMode::Exploration => {
            dispatch_action(store, Action::Interact);
            false
        }
        _ => false,
    }
}

fn dispatch_action(store: &mut EffectStore<AppState, Action, Effect>, action: Action) {
    let mut queue = VecDeque::from([action]);

    while let Some(next_action) = queue.pop_front() {
        let result = store.dispatch(next_action);
        for effect in result.effects {
            handle_effect(store, effect, &mut queue);
        }
    }
}

fn handle_effect(
    store: &mut EffectStore<AppState, Action, Effect>,
    effect: Effect,
    queue: &mut VecDeque<Action>,
) {
    match effect {
        Effect::GenerateFloor {
            floor_index,
            seed,
            width,
            height,
        } => match procgen::generate_floor(seed, floor_index, width, height) {
            Ok(floor) => queue.push_back(Action::FloorGenerated(floor)),
            Err(err) => {
                let state = store.state_mut();
                state.mode = GameMode::GameOver;
                state.last_status = Some(format!("Floor generation failed: {err}"));
            }
        },
    }
}
