use std::collections::HashMap;
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
    prev_entries: HashMap<u32, SpriteEntry>,
}

impl<W: Write> SpriteBackend<W> {
    pub fn new(writer: W, registry: Arc<Mutex<SpriteRegistry>>) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            registry,
            prev_entries: HashMap::new(),
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
        let mut current_entries = HashMap::with_capacity(sprites.len());
        for entry in sprites {
            current_entries.insert(entry.id, entry);
        }

        for (id, prev) in &self.prev_entries {
            match current_entries.get(id) {
                None => {
                    queue!(self.inner, Print(format!("\x1b_Ga=d,d=i,i={id}\x1b\\")))?;
                }
                Some(cur) if cur.x != prev.x || cur.y != prev.y || cur.data != prev.data => {
                    queue!(self.inner, Print(format!("\x1b_Ga=d,d=i,i={id}\x1b\\")))?;
                }
                _ => {}
            }
        }

        for (id, cur) in &current_entries {
            match self.prev_entries.get(id) {
                None => {
                    queue!(self.inner, MoveTo(cur.x, cur.y), Print(&cur.data))?;
                }
                Some(prev) if cur.x != prev.x || cur.y != prev.y || cur.data != prev.data => {
                    queue!(self.inner, MoveTo(cur.x, cur.y), Print(&cur.data))?;
                }
                _ => {}
            }
        }

        self.prev_entries = current_entries;
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
