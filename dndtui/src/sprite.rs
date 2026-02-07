use std::io::Cursor;

use base64::{engine::general_purpose, Engine as _};
use image::{codecs::gif::GifDecoder, AnimationDecoder, GenericImageView};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpriteFrame {
    pub payload: String,
    pub width: u32,
    pub height: u32,
    pub format: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpriteData {
    pub frames: Vec<SpriteFrame>,
    pub width: u32,
    pub height: u32,
}

impl SpriteData {
    pub fn frame(&self, index: usize) -> &SpriteFrame {
        let idx = if self.frames.is_empty() {
            0
        } else {
            index % self.frames.len()
        };
        &self.frames[idx]
    }

    /// Returns a horizontally flipped version of this sprite.
    pub fn flipped(&self) -> SpriteData {
        SpriteData {
            frames: self.frames.iter().map(|f| f.flipped()).collect(),
            width: self.width,
            height: self.height,
        }
    }
}

impl SpriteFrame {
    /// Returns a horizontally flipped version of this frame.
    pub fn flipped(&self) -> SpriteFrame {
        let bytes = match general_purpose::STANDARD.decode(&self.payload) {
            Ok(b) => b,
            Err(_) => return self.clone(),
        };

        if self.format == 32 {
            let mut flipped = bytes.clone();
            let stride = (self.width * 4) as usize;
            for y in 0..self.height as usize {
                let row_start = y * stride;
                for x in 0..(self.width as usize / 2) {
                    let left = row_start + x * 4;
                    let right = row_start + (self.width as usize - 1 - x) * 4;
                    for i in 0..4 {
                        flipped.swap(left + i, right + i);
                    }
                }
            }
            SpriteFrame {
                payload: general_purpose::STANDARD.encode(&flipped),
                width: self.width,
                height: self.height,
                format: self.format,
            }
        } else if let Ok(img) = image::load_from_memory(&bytes) {
            let flipped_img = image::imageops::flip_horizontal(&img);
            let mut buf = Vec::new();
            if flipped_img
                .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
                .is_ok()
            {
                return SpriteFrame {
                    payload: general_purpose::STANDARD.encode(&buf),
                    width: self.width,
                    height: self.height,
                    format: self.format,
                };
            }
            self.clone()
        } else {
            self.clone()
        }
    }
}

pub fn decode_sprite(bytes: &[u8], url: &str) -> Result<SpriteData, String> {
    if is_gif(bytes, url) {
        let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(|err| err.to_string())?;
        let frames = decoder
            .into_frames()
            .collect_frames()
            .map_err(|err| err.to_string())?;
        let mut sprite_frames = Vec::new();
        for frame in frames {
            let buffer = frame.into_buffer();
            let (width, height) = buffer.dimensions();
            let encoded = general_purpose::STANDARD.encode(buffer.as_raw());
            sprite_frames.push(SpriteFrame {
                payload: encoded,
                width,
                height,
                format: 32,
            });
        }
        if let Some(first) = sprite_frames.first() {
            let (width, height) = (first.width, first.height);
            return Ok(SpriteData {
                frames: sprite_frames,
                width,
                height,
            });
        }
    }

    let image = image::load_from_memory(bytes).map_err(|err| err.to_string())?;
    let (width, height) = image.dimensions();
    let encoded = general_purpose::STANDARD.encode(bytes);
    Ok(SpriteData {
        frames: vec![SpriteFrame {
            payload: encoded,
            width,
            height,
            format: 100,
        }],
        width,
        height,
    })
}

pub fn kitty_sequence(
    frame: &SpriteFrame,
    cols: u16,
    rows: u16,
    id: u32,
) -> Result<String, String> {
    let mut sequences = String::new();
    let chunk_size = 4096;
    let payload = frame.payload.as_bytes();
    let total_chunks = (payload.len() + chunk_size - 1) / chunk_size;

    for (index, chunk) in payload.chunks(chunk_size).enumerate() {
        let more = index + 1 < total_chunks;
        if index == 0 {
            let mut params = format!(
                "f={},s={},v={},a=T,t=d,i={}",
                frame.format, frame.width, frame.height, id
            );
            if cols > 0 {
                params.push_str(&format!(",c={cols}"));
            }
            if rows > 0 {
                params.push_str(&format!(",r={rows}"));
            }
            params.push_str(&format!(",m={}", if more { 1 } else { 0 }));
            let chunk_str = std::str::from_utf8(chunk).map_err(|err| err.to_string())?;
            sequences.push_str(&format!("\x1b_G{params};{chunk_str}\x1b\\"));
        } else {
            let chunk_str = std::str::from_utf8(chunk).map_err(|err| err.to_string())?;
            sequences.push_str(&format!(
                "\x1b_Gm={};{chunk_str}\x1b\\",
                if more { 1 } else { 0 }
            ));
        }
    }
    Ok(sequences)
}

fn is_gif(bytes: &[u8], url: &str) -> bool {
    if url.ends_with(".gif") {
        return true;
    }
    bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")
}
