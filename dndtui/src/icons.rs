use std::path::PathBuf;
use std::sync::OnceLock;

use crate::sprite::{decode_sprite, SpriteData};

#[derive(Clone, Debug, Default)]
pub struct IconSet {
    pub player: Option<SpriteData>,
    pub npc: Option<SpriteData>,
    pub encounter: Option<SpriteData>,
    pub item: Option<SpriteData>,
}

static ICONS: OnceLock<IconSet> = OnceLock::new();

pub fn icon_set() -> &'static IconSet {
    ICONS.get_or_init(load_icons)
}

fn load_icons() -> IconSet {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
    IconSet {
        player: load_icon(base.join("player.png")),
        npc: load_icon(base.join("npc.png")),
        encounter: load_icon(base.join("encounter.png")),
        item: load_icon(base.join("item.png")),
    }
}

fn load_icon(path: PathBuf) -> Option<SpriteData> {
    let bytes = std::fs::read(&path).ok()?;
    let url = path.to_string_lossy();
    decode_sprite(&bytes, url.as_ref()).ok()
}
