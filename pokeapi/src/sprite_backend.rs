use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, OnceLock};

use crossterm::{cursor::MoveTo, queue, style::Print};
use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

#[derive(Default, Debug, Clone)]
pub struct SpriteRegistry {
    sprites: HashMap<(u16, u16), String>,
}

impl SpriteRegistry {
    pub fn set(&mut self, x: u16, y: u16, data: String) {
        self.sprites.insert((x, y), data);
    }

    pub fn clear(&mut self) {
        self.sprites.clear();
    }

    pub fn entries(&self) -> Vec<((u16, u16), String)> {
        self.sprites
            .iter()
            .map(|(pos, data)| (*pos, data.clone()))
            .collect()
    }
}

static REGISTRY: OnceLock<Arc<Mutex<SpriteRegistry>>> = OnceLock::new();

pub fn sprite_registry() -> Arc<Mutex<SpriteRegistry>> {
    REGISTRY
        .get_or_init(|| Arc::new(Mutex::new(SpriteRegistry::default())))
        .clone()
}

pub fn update_sprite(x: u16, y: u16, data: String) {
    let registry = sprite_registry();
    let mut registry = registry.lock().expect("sprite registry lock");
    registry.clear();
    registry.set(x, y, data);
}

pub fn clear_sprites() {
    let registry = sprite_registry();
    let mut registry = registry.lock().expect("sprite registry lock");
    registry.clear();
}

#[derive(Debug, Clone)]
pub struct SpriteBackend<W: Write> {
    inner: CrosstermBackend<W>,
    registry: Arc<Mutex<SpriteRegistry>>,
    had_sprite: bool,
}

impl<W: Write> SpriteBackend<W> {
    pub fn new(writer: W, registry: Arc<Mutex<SpriteRegistry>>) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            registry,
            had_sprite: false,
        }
    }
}

impl<W: Write> Backend for SpriteBackend<W> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)?;
        let sprites = {
            let registry = self.registry.lock().expect("sprite registry lock");
            registry.entries()
        };
        if self.had_sprite {
            queue!(self.inner, Print("\x1b_Ga=d,d=a\x1b\\"))?;
        }
        if sprites.is_empty() {
            self.had_sprite = false;
            return Ok(());
        }
        for ((x, y), data) in sprites {
            queue!(self.inner, MoveTo(x, y), Print(data))?;
        }
        self.had_sprite = true;
        Ok(())
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

impl<W: Write> Write for SpriteBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut self.inner)
    }
}
