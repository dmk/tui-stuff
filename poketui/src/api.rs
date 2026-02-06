use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::fs;

use crate::state::PokemonInfo;

const API_BASE: &str = "https://pokeapi.co/api/v2";

#[derive(Clone, Debug, Deserialize)]
struct NamedResource {
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonResponse {
    name: String,
    base_experience: Option<u16>,
    stats: Vec<PokemonStatSlot>,
    sprites: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonStatSlot {
    base_stat: u16,
    stat: NamedResource,
}

pub async fn fetch_pokemon(name: &str) -> Result<PokemonInfo, String> {
    let url = format!("{API_BASE}/pokemon/{name}");
    let response: PokemonResponse = fetch_json_cached(&url).await?;

    let get_stat = |stat_name: &str| -> u16 {
        response
            .stats
            .iter()
            .find(|slot| slot.stat.name == stat_name)
            .map(|slot| slot.base_stat)
            .unwrap_or(35)
    };

    Ok(PokemonInfo {
        name: response.name,
        base_experience: response.base_experience.unwrap_or(60),
        hp: get_stat("hp"),
        attack: get_stat("attack"),
        defense: get_stat("defense"),
        sp_attack: get_stat("special-attack"),
        sp_defense: get_stat("special-defense"),
        speed: get_stat("speed"),
        sprite_front_default: pointer_string(&response.sprites, "/front_default"),
        sprite_back_default: pointer_string(&response.sprites, "/back_default"),
        sprite_front_animated: pointer_string(
            &response.sprites,
            "/versions/generation-v/black-white/animated/front_default",
        ),
        sprite_back_animated: pointer_string(
            &response.sprites,
            "/versions/generation-v/black-white/animated/back_default",
        ),
    })
}

pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    fetch_bytes_cached(url).await
}

fn pointer_string(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(|val| val.as_str())
        .map(|s| s.to_string())
}

async fn fetch_json_cached<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let bytes = fetch_bytes_cached(url).await?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(value),
        Err(err) => {
            let cache_path = cache_path("http", url);
            let _ = fs::remove_file(&cache_path).await;
            Err(err.to_string())
        }
    }
}

async fn fetch_bytes_cached(url: &str) -> Result<Vec<u8>, String> {
    let cache_path = cache_path("http", url);
    if let Some(bytes) = read_cache(&cache_path).await {
        return Ok(bytes);
    }

    let client = http_client();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let response = response.error_for_status().map_err(|err| err.to_string())?;
    let bytes = response
        .bytes()
        .await
        .map_err(|err| err.to_string())?
        .to_vec();
    write_cache(&cache_path, &bytes).await;
    Ok(bytes)
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

fn cache_root() -> PathBuf {
    let base = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join(".cache").join("poketui")
}

fn cache_path(kind: &str, url: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let digest = hex::encode(hasher.finalize());
    cache_root().join(kind).join(digest)
}

async fn read_cache(path: &Path) -> Option<Vec<u8>> {
    if let Ok(bytes) = fs::read(path).await {
        return Some(bytes);
    }
    None
}

async fn write_cache(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    let _ = fs::write(path, bytes).await;
}
