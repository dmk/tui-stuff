use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crossterm::{
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::supports_keyboard_enhancement,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum KeyboardMode {
    #[default]
    Auto,
    Held,
    Legacy,
}

/// Negotiate before the runtime starts reading terminal events, and restore the
/// keyboard protocol while still in the alternate screen on normal/error exits.
pub struct KeyboardSession {
    mode: KeyboardMode,
    active: Arc<AtomicBool>,
}

impl KeyboardSession {
    pub fn start(requested: KeyboardMode) -> io::Result<Self> {
        let held = match requested {
            KeyboardMode::Auto => supports_keyboard_enhancement().unwrap_or(false),
            KeyboardMode::Held => true,
            KeyboardMode::Legacy => false,
        };
        let active = Arc::new(AtomicBool::new(false));
        if held {
            execute!(
                io::stdout(),
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
                )
            )?;
            active.store(true, Ordering::SeqCst);
            // Pop before Ratatui's panic hook leaves the alternate screen.
            // The atomic flag prevents a second pop during stack unwinding.
            let previous_hook = std::panic::take_hook();
            let panic_active = Arc::clone(&active);
            std::panic::set_hook(Box::new(move |info| {
                if panic_active.swap(false, Ordering::SeqCst) {
                    let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
                }
                previous_hook(info);
            }));
        }
        Ok(Self {
            mode: if held {
                KeyboardMode::Held
            } else {
                KeyboardMode::Legacy
            },
            active,
        })
    }

    pub fn mode(&self) -> KeyboardMode {
        self.mode
    }
}

impl Drop for KeyboardSession {
    fn drop(&mut self) {
        if self.active.swap(false, Ordering::SeqCst) {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        }
    }
}
