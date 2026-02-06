use std::collections::HashSet;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, OnceLock};

use crossterm::{cursor::MoveTo, queue, style::Print};
use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

#[derive(Debug, Clone)]
pub struct SpriteEntry {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub data: String,
}

#[derive(Default, Debug, Clone)]
pub struct SpriteRegistry {
    sprites: Vec<SpriteEntry>,
}

impl SpriteRegistry {
    pub fn set(&mut self, id: u32, x: u16, y: u16, data: String) {
        self.sprites.push(SpriteEntry { id, x, y, data });
    }

    pub fn clear(&mut self) {
        self.sprites.clear();
    }

    pub fn entries(&self) -> Vec<SpriteEntry> {
        self.sprites.clone()
    }
}

static REGISTRY: OnceLock<Arc<Mutex<SpriteRegistry>>> = OnceLock::new();

pub fn sprite_registry() -> Arc<Mutex<SpriteRegistry>> {
    REGISTRY
        .get_or_init(|| Arc::new(Mutex::new(SpriteRegistry::default())))
        .clone()
}

pub fn set_sprite(id: u32, x: u16, y: u16, data: String) {
    let registry = sprite_registry();
    let mut registry = registry.lock().expect("sprite registry lock");
    registry.set(id, x, y, data);
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
    prev_ids: HashSet<u32>,
}

impl<W: Write> SpriteBackend<W> {
    pub fn new(writer: W, registry: Arc<Mutex<SpriteRegistry>>) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            registry,
            prev_ids: HashSet::new(),
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
        let current_ids: HashSet<u32> = sprites.iter().map(|e| e.id).collect();
        // Delete ALL previous sprites to clear old positions when sprites move
        for &id in &self.prev_ids {
            queue!(self.inner, Print(format!("\x1b_Ga=d,d=i,i={id}\x1b\\")))?;
        }
        for entry in &sprites {
            queue!(self.inner, MoveTo(entry.x, entry.y), Print(&entry.data))?;
        }
        self.prev_ids = current_ids;
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
