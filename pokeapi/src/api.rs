use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::state::{
    AbilityDetail, EncounterDetail, EncounterLocation, EncounterVersion, EvolutionChain, MoveDetail,
    PokedexEntry, PokemonDetail, PokemonSpecies, PokemonStat, RegionInfo, TypeMatchup,
};

const API_BASE: &str = "https://pokeapi.co/api/v2";
const SPECIES_INDEX_CONCURRENCY: usize = 12;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct NamedResource {
    name: String,
    url: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PokedexResponse {
    pokemon_entries: Vec<PokedexEntryResponse>,
}

#[derive(Clone, Debug, Deserialize)]
struct PokedexEntryResponse {
    entry_number: u16,
    pokemon_species: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct ListResponse {
    results: Vec<NamedResource>,
}

#[derive(Clone, Debug, Deserialize)]
struct TypeListResponse {
    results: Vec<NamedResource>,
}

#[derive(Clone, Debug, Deserialize)]
struct TypeDetailResponse {
    pokemon: Vec<TypePokemonEntry>,
    damage_relations: DamageRelations,
}

#[derive(Clone, Debug, Deserialize)]
struct DamageRelations {
    double_damage_from: Vec<NamedResource>,
    double_damage_to: Vec<NamedResource>,
    half_damage_from: Vec<NamedResource>,
    half_damage_to: Vec<NamedResource>,
    no_damage_from: Vec<NamedResource>,
    no_damage_to: Vec<NamedResource>,
}

#[derive(Clone, Debug, Deserialize)]
struct TypePokemonEntry {
    pokemon: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonResponse {
    id: u16,
    name: String,
    height: u16,
    weight: u16,
    types: Vec<PokemonTypeSlot>,
    stats: Vec<PokemonStatSlot>,
    abilities: Vec<PokemonAbilitySlot>,
    moves: Vec<PokemonMoveSlot>,
    sprites: serde_json::Value,
    cries: Option<PokemonCries>,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonTypeSlot {
    #[serde(rename = "type")]
    type_info: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonStatSlot {
    base_stat: u16,
    stat: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonAbilitySlot {
    ability: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonMoveSlot {
    #[serde(rename = "move")]
    move_info: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonCries {
    latest: Option<String>,
    legacy: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct MoveDetailResponse {
    name: String,
    power: Option<u16>,
    accuracy: Option<u16>,
    pp: Option<u16>,
    effect_entries: Vec<EffectEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct AbilityDetailResponse {
    name: String,
    effect_entries: Vec<EffectEntry>,
}

#[derive(Clone, Debug, Deserialize)]
struct EffectEntry {
    effect: String,
    short_effect: String,
    language: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct PokemonSpeciesResponse {
    name: String,
    flavor_text_entries: Vec<FlavorTextEntry>,
    genera: Vec<GenusEntry>,
    evolution_chain: Option<ApiResource>,
    evolves_from_species: Option<NamedResource>,
}

#[derive(Clone, Debug, Deserialize)]
struct EncounterLocationResponse {
    location_area: NamedResource,
    version_details: Vec<EncounterVersionDetailResponse>,
}

#[derive(Clone, Debug, Deserialize)]
struct EncounterVersionDetailResponse {
    version: NamedResource,
    max_chance: u8,
    encounter_details: Vec<EncounterDetailResponse>,
}

#[derive(Clone, Debug, Deserialize)]
struct EncounterDetailResponse {
    min_level: u8,
    max_level: u8,
    method: NamedResource,
    chance: u8,
    condition_values: Vec<NamedResource>,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiResource {
    url: String,
}

#[derive(Clone, Debug, Deserialize)]
struct FlavorTextEntry {
    flavor_text: String,
    language: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct GenusEntry {
    genus: String,
    language: NamedResource,
}

#[derive(Clone, Debug, Deserialize)]
struct EvolutionChainResponse {
    chain: ChainLink,
}

#[derive(Clone, Debug, Deserialize)]
struct ChainLink {
    species: NamedResource,
    evolves_to: Vec<ChainLink>,
}

pub async fn fetch_pokedex(name: &str) -> Result<Vec<PokedexEntry>, String> {
    let url = format!("{API_BASE}/pokedex/{name}");
    let response: PokedexResponse = fetch_json_cached(&url).await?;
    let mut entries: Vec<PokedexEntry> = response
        .pokemon_entries
        .into_iter()
        .map(|entry| PokedexEntry {
            entry_number: entry.entry_number,
            name: entry.pokemon_species.name,
            url: entry.pokemon_species.url,
        })
        .collect();
    entries.sort_by_key(|entry| entry.entry_number);
    Ok(entries)
}

pub async fn fetch_regions() -> Result<Vec<RegionInfo>, String> {
    let url = format!("{API_BASE}/pokedex?limit=200");
    let response: ListResponse = fetch_json_cached(&url).await?;
    let mut regions: Vec<RegionInfo> = response
        .results
        .into_iter()
        .filter_map(|entry| {
            let label = format_region_label(&entry.name)?;
            Some(RegionInfo {
                name: entry.name,
                label,
            })
        })
        .collect();

    regions.sort_by_key(|region| region_sort_key(&region.name));
    Ok(regions)
}

pub async fn fetch_type_list() -> Result<Vec<String>, String> {
    let url = format!("{API_BASE}/type?limit=999");
    let response: TypeListResponse = fetch_json_cached(&url).await?;
    let mut types: Vec<String> = response
        .results
        .into_iter()
        .map(|entry| entry.name)
        .filter(|name| name != "unknown" && name != "shadow")
        .collect();
    types.sort();
    Ok(types)
}

pub async fn fetch_type_detail(name: &str) -> Result<Vec<String>, String> {
    let url = format!("{API_BASE}/type/{name}");
    let response: TypeDetailResponse = fetch_json_cached(&url).await?;
    Ok(response
        .pokemon
        .into_iter()
        .map(|entry| entry.pokemon.name)
        .collect())
}

pub async fn fetch_type_matchup(name: &str) -> Result<TypeMatchup, String> {
    let url = format!("{API_BASE}/type/{name}");
    let response: TypeDetailResponse = fetch_json_cached(&url).await?;
    Ok(TypeMatchup {
        name: name.to_string(),
        double_from: response
            .damage_relations
            .double_damage_from
            .into_iter()
            .map(|entry| entry.name)
            .collect(),
        half_from: response
            .damage_relations
            .half_damage_from
            .into_iter()
            .map(|entry| entry.name)
            .collect(),
        no_from: response
            .damage_relations
            .no_damage_from
            .into_iter()
            .map(|entry| entry.name)
            .collect(),
        double_to: response
            .damage_relations
            .double_damage_to
            .into_iter()
            .map(|entry| entry.name)
            .collect(),
        half_to: response
            .damage_relations
            .half_damage_to
            .into_iter()
            .map(|entry| entry.name)
            .collect(),
        no_to: response
            .damage_relations
            .no_damage_to
            .into_iter()
            .map(|entry| entry.name)
            .collect(),
    })
}

pub async fn fetch_pokemon_detail(name: &str) -> Result<PokemonDetail, String> {
    let url = format!("{API_BASE}/pokemon/{name}");
    let response: PokemonResponse = fetch_json_cached(&url).await?;

    let types = response
        .types
        .into_iter()
        .map(|slot| slot.type_info.name)
        .collect();
    let stats = response
        .stats
        .into_iter()
        .map(|slot| PokemonStat {
            name: slot.stat.name,
            value: slot.base_stat,
        })
        .collect();
    let abilities = response
        .abilities
        .into_iter()
        .map(|slot| slot.ability.name)
        .collect();
    let moves = response
        .moves
        .into_iter()
        .map(|slot| slot.move_info.name)
        .collect();

    let sprite_front_default = pointer_string(&response.sprites, "/front_default");
    let sprite_front_shiny = pointer_string(&response.sprites, "/front_shiny");
    let sprite_animated = pointer_string(
        &response.sprites,
        "/versions/generation-v/black-white/animated/front_default",
    );

    Ok(PokemonDetail {
        id: response.id,
        name: response.name,
        types,
        stats,
        abilities,
        moves,
        height: response.height,
        weight: response.weight,
        sprite_front_default,
        sprite_front_shiny,
        sprite_animated,
        cries_latest: response.cries.as_ref().and_then(|cries| cries.latest.clone()),
        cries_legacy: response.cries.as_ref().and_then(|cries| cries.legacy.clone()),
    })
}

pub async fn fetch_pokemon_species(name: &str) -> Result<PokemonSpecies, String> {
    let url = format!("{API_BASE}/pokemon-species/{name}");
    let response: PokemonSpeciesResponse = fetch_json_cached(&url).await?;
    let flavor_text = response
        .flavor_text_entries
        .iter()
        .find(|entry| entry.language.name == "en")
        .map(|entry| sanitize_text(&entry.flavor_text));
    let genus = response
        .genera
        .iter()
        .find(|entry| entry.language.name == "en")
        .map(|entry| entry.genus.clone());
    Ok(PokemonSpecies {
        name: response.name,
        flavor_text,
        genus,
        evolution_chain_url: response.evolution_chain.map(|chain| chain.url),
        evolves_from: response
            .evolves_from_species
            .map(|species| species.name),
    })
}

pub async fn fetch_pokemon_encounters(name: &str) -> Result<Vec<EncounterLocation>, String> {
    let url = format!("{API_BASE}/pokemon/{name}/encounters");
    let response: Vec<EncounterLocationResponse> = fetch_json_cached(&url).await?;
    let encounters = response
        .into_iter()
        .map(|location| EncounterLocation {
            location: location.location_area.name,
            version_details: location
                .version_details
                .into_iter()
                .map(|version| EncounterVersion {
                    version: version.version.name,
                    max_chance: version.max_chance,
                    encounters: version
                        .encounter_details
                        .into_iter()
                        .map(|detail| EncounterDetail {
                            min_level: detail.min_level,
                            max_level: detail.max_level,
                            method: detail.method.name,
                            chance: detail.chance,
                            conditions: detail
                                .condition_values
                                .into_iter()
                                .map(|condition| condition.name)
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    Ok(encounters)
}

pub async fn fetch_species_index(names: &[String]) -> Result<Vec<PokemonSpecies>, String> {
    if names.is_empty() {
        return Ok(Vec::new());
    }

    let semaphore = Arc::new(Semaphore::new(SPECIES_INDEX_CONCURRENCY));
    let mut join_set = JoinSet::new();
    for name in names {
        let name = name.clone();
        let semaphore = semaphore.clone();
        join_set.spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .map_err(|_| "Species index semaphore closed".to_string())?;
            fetch_pokemon_species(&name).await
        });
    }

    let mut species = Vec::with_capacity(names.len());
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(entry)) => species.push(entry),
            Ok(Err(_)) => {}
            Err(_) => {}
        }
    }

    if species.is_empty() {
        return Err("Failed to load species index".to_string());
    }
    Ok(species)
}

pub async fn fetch_move_detail(name: &str) -> Result<MoveDetail, String> {
    let url = format!("{API_BASE}/move/{name}");
    let response: MoveDetailResponse = fetch_json_cached(&url).await?;
    Ok(MoveDetail {
        name: response.name,
        power: response.power,
        accuracy: response.accuracy,
        pp: response.pp,
        effect: effect_text(&response.effect_entries),
    })
}

pub async fn fetch_ability_detail(name: &str) -> Result<AbilityDetail, String> {
    let url = format!("{API_BASE}/ability/{name}");
    let response: AbilityDetailResponse = fetch_json_cached(&url).await?;
    Ok(AbilityDetail {
        name: response.name,
        effect: effect_text(&response.effect_entries),
    })
}

pub async fn fetch_evolution_chain(id: &str, url: &str) -> Result<EvolutionChain, String> {
    let response: EvolutionChainResponse = fetch_json_cached(url).await?;
    let mut stages = Vec::new();
    build_chain_stages(&response.chain, &mut stages);
    Ok(EvolutionChain {
        id: id.to_string(),
        stages,
    })
}

pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    fetch_bytes_cached(url).await
}

fn sanitize_text(text: &str) -> String {
    text.replace('\n', " ").replace('\u{000C}', " ")
}

fn effect_text(entries: &[EffectEntry]) -> Option<String> {
    entries
        .iter()
        .find(|entry| entry.language.name == "en")
        .map(|entry| {
            let text = if entry.short_effect.is_empty() {
                &entry.effect
            } else {
                &entry.short_effect
            };
            sanitize_text(text)
        })
}

fn pointer_string(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(|val| val.as_str())
        .map(|s| s.to_string())
}

fn format_region_label(name: &str) -> Option<String> {
    if name == "national" || name.contains("conquest") {
        return None;
    }
    let label = name.replace('-', " ").to_ascii_uppercase();
    Some(label)
}

fn region_sort_key(name: &str) -> (usize, String) {
    let order = [
        "kanto",
        "original-johto",
        "hoenn",
        "sinnoh",
        "unova",
        "kalos-central",
        "kalos-coastal",
        "kalos-mountain",
        "alola",
        "melemele",
        "akala",
        "ulaula",
        "poni",
        "galar",
        "isle-of-armor",
        "crown-tundra",
        "hisui",
        "paldea",
    ];
    let index = order
        .iter()
        .position(|item| item == &name)
        .unwrap_or(order.len());
    (index, name.to_string())
}

fn build_chain_stages(chain: &ChainLink, stages: &mut Vec<String>) {
    if !stages.contains(&chain.species.name) {
        stages.push(chain.species.name.clone());
    }
    for next in &chain.evolves_to {
        build_chain_stages(next, stages);
    }
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
    base.join(".cache").join("pokeapi-tui")
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
