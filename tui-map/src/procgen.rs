use std::fmt;

use crate::core::{MapGrid, TileKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerateRequest<P> {
    pub generator_id: String,
    pub generator_version: u32,
    pub seed: u64,
    pub width: u16,
    pub height: u16,
    pub params: P,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnchorKind {
    PlayerStart,
    Npc,
    Item,
    Encounter,
    Trigger,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnAnchor {
    pub kind: AnchorKind,
    pub x: u16,
    pub y: u16,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationFingerprint {
    pub generator_id: String,
    pub generator_version: u32,
    pub seed: u64,
    pub output_hash_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedMap {
    pub map: MapGrid,
    pub anchors: Vec<SpawnAnchor>,
    pub fingerprint: GenerationFingerprint,
}

impl GeneratedMap {
    pub fn with_computed_fingerprint(
        generator_id: impl Into<String>,
        generator_version: u32,
        seed: u64,
        map: MapGrid,
        anchors: Vec<SpawnAnchor>,
    ) -> Self {
        let generator_id = generator_id.into();
        let fingerprint = compute_fingerprint(&generator_id, generator_version, seed, &map, &anchors);
        Self {
            map,
            anchors,
            fingerprint,
        }
    }
}

pub trait MapGenerator<P>: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> u32;
    fn generate(&self, req: &GenerateRequest<P>) -> Result<GeneratedMap, GenError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenError {
    InvalidParams(String),
    InvalidSize,
    Internal(String),
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenError::InvalidParams(msg) => write!(f, "invalid params: {}", msg),
            GenError::InvalidSize => write!(f, "invalid map size"),
            GenError::Internal(msg) => write!(f, "internal generation error: {}", msg),
        }
    }
}

impl std::error::Error for GenError {}

pub fn compute_fingerprint(
    generator_id: &str,
    generator_version: u32,
    seed: u64,
    map: &MapGrid,
    anchors: &[SpawnAnchor],
) -> GenerationFingerprint {
    let mut hash = 1469598103934665603u64;

    fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
        for byte in bytes {
            *hash ^= *byte as u64;
            *hash = hash.wrapping_mul(1099511628211);
        }
    }

    fn hash_str(hash: &mut u64, value: &str) {
        hash_bytes(hash, value.as_bytes());
        hash_bytes(hash, &[0xff]);
    }

    fn hash_u16(hash: &mut u64, value: u16) {
        hash_bytes(hash, &value.to_le_bytes());
    }

    fn hash_u32(hash: &mut u64, value: u32) {
        hash_bytes(hash, &value.to_le_bytes());
    }

    fn hash_u64(hash: &mut u64, value: u64) {
        hash_bytes(hash, &value.to_le_bytes());
    }

    fn hash_tile(hash: &mut u64, tile: TileKind) {
        match tile {
            TileKind::Grass => hash_bytes(hash, &[0]),
            TileKind::Trail => hash_bytes(hash, &[1]),
            TileKind::Sand => hash_bytes(hash, &[2]),
            TileKind::Floor => hash_bytes(hash, &[3]),
            TileKind::Wall => hash_bytes(hash, &[4]),
            TileKind::Water => hash_bytes(hash, &[5]),
            TileKind::Custom(id) => {
                hash_bytes(hash, &[6]);
                hash_u16(hash, id);
            }
        }
    }

    fn hash_anchor_kind(hash: &mut u64, kind: &AnchorKind) {
        match kind {
            AnchorKind::PlayerStart => hash_bytes(hash, &[10]),
            AnchorKind::Npc => hash_bytes(hash, &[11]),
            AnchorKind::Item => hash_bytes(hash, &[12]),
            AnchorKind::Encounter => hash_bytes(hash, &[13]),
            AnchorKind::Trigger => hash_bytes(hash, &[14]),
            AnchorKind::Custom(label) => {
                hash_bytes(hash, &[15]);
                hash_str(hash, label);
            }
        }
    }

    hash_str(&mut hash, generator_id);
    hash_u32(&mut hash, generator_version);
    hash_u64(&mut hash, seed);

    hash_str(&mut hash, &map.name);
    hash_u16(&mut hash, map.size.width);
    hash_u16(&mut hash, map.size.height);
    for tile in &map.tiles {
        hash_tile(&mut hash, *tile);
    }

    hash_u64(&mut hash, anchors.len() as u64);
    for anchor in anchors {
        hash_anchor_kind(&mut hash, &anchor.kind);
        hash_u16(&mut hash, anchor.x);
        hash_u16(&mut hash, anchor.y);
        hash_str(&mut hash, anchor.tag.as_deref().unwrap_or(""));
    }

    GenerationFingerprint {
        generator_id: generator_id.to_string(),
        generator_version,
        seed,
        output_hash_hex: format!("{:016x}", hash),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::MapSize;

    #[test]
    fn fingerprint_is_stable_for_same_input() {
        let map = MapGrid::filled("demo", MapSize::new(4, 4), TileKind::Grass);
        let anchors = vec![SpawnAnchor {
            kind: AnchorKind::PlayerStart,
            x: 2,
            y: 2,
            tag: None,
        }];

        let a = compute_fingerprint("demo-gen", 1, 42, &map, &anchors);
        let b = compute_fingerprint("demo-gen", 1, 42, &map, &anchors);
        assert_eq!(a.output_hash_hex, b.output_hash_hex);
    }

    #[test]
    fn fingerprint_changes_when_version_changes() {
        let map = MapGrid::filled("demo", MapSize::new(4, 4), TileKind::Grass);
        let anchors = vec![SpawnAnchor {
            kind: AnchorKind::PlayerStart,
            x: 2,
            y: 2,
            tag: None,
        }];

        let v1 = compute_fingerprint("demo-gen", 1, 42, &map, &anchors);
        let v2 = compute_fingerprint("demo-gen", 2, 42, &map, &anchors);
        assert_ne!(v1.output_hash_hex, v2.output_hash_hex);
    }
}
