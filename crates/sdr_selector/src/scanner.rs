use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    net::Ipv4Addr,
    path::Path,
};

use std::convert::From;

use serde::{Deserialize, Serialize};

use crate::{Result, games::SteamGame, system::get_cache_dir};

const RELAY_API: &str = "https://api.steampowered.com/ISteamApps/GetSDRConfig/v1?appid={}";
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelayId(pub u64);

impl From<u64> for RelayId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<&RelayId> for u64 {
    fn from(value: &RelayId) -> Self {
        value.0
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterResponse {
    pub pops: HashMap<String, Pop>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pop {
    #[serde(skip)]
    pub sname: String,

    pub desc: String,
    #[serde(default)]
    pub relays: Vec<Relay>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Relay {
    pub ipv4: Ipv4Addr,
    pub port_range: (u16, u16),
}

pub async fn fetch_save_game_relays_cache<G: SteamGame>(appid: &G) -> Result<()> {
    let mut save_path = get_cache_dir();
    std::fs::create_dir_all(&save_path).ok();
    save_path.push(appid.cache_name());
    let text_body = get_relay_api(appid).await?;
    save_game_relays(&text_body, save_path)
}

pub fn save_game_relays<A: AsRef<Path>>(text: &str, path: A) -> Result<()> {
    let cluster: ClusterResponse = serde_json::from_str(text)?;
    let pops = cluster.pops;
    let json = serde_json::to_string_pretty(&pops)?;
    std::fs::write(path, json)?;

    Ok(())
}

pub fn read_cluster_cache<P: AsRef<Path>>(path: P) -> Result<HashMap<RelayId, Pop>> {
    let text = std::fs::read_to_string(path)?;
    let raw_pop: HashMap<String, Pop> = serde_json::from_str(&text)?;
    let mut relay_map: HashMap<RelayId, Pop> = HashMap::with_capacity(raw_pop.len());

    for (key, mut pop) in raw_pop {
        pop.sname = key.clone();
        let mut hasher = DefaultHasher::new();
        pop.sname.hash(&mut hasher);
        let id: u64 = hasher.finish();
        relay_map.insert(id.into(), pop);
    }

    Ok(relay_map)
}

pub fn read_game_cluster_cache<G: SteamGame>(game: G) -> Result<HashMap<RelayId, Pop>> {
    let mut save_path = get_cache_dir();
    save_path.push(format!("{}.json", game.id()));
    read_cluster_cache(save_path)
}

pub async fn get_relay_api<G: SteamGame>(appid: &G) -> Result<String> {
    let api_url = format!("{}", RELAY_API.replace("{}", &appid.id().to_string()));
    let response = reqwest::get(&api_url).await?;
    Ok(response.text().await?)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, println};

    use crate::games::GameID;

    use super::*;

    #[tokio::test]
    async fn test_save_cs2_relays() {
        let text = include_str!("../clusters.json");
        let r = save_game_relays(text, Path::new("clusters_test.json"));
        if let Err(err) = r {
            println!("{}", err);
        }
        let path = Path::new("clusters_test.json");
        let result = std::fs::read_to_string(path);
        assert!(result.is_ok());
        if let Ok(f) = result {
            println!("{}", f);
        }
        // std::fs::remove_file(path).ok();
    }
    #[tokio::test]
    async fn test_read() {
        let text = include_str!("../clusters.json");
        let result = read_cluster_cache("clusters_test.json");
        if let Ok(f) = &result {
            for (id, pop) in f {
                println!("{}", pop.desc);
            }
        }
        if let Err(f) = &result {
            println!("{}", f);
        }

        assert!(result.is_ok());
        // std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn test_fetch() {
        let game = GameID::DeadLock;
        let data = get_relay_api(&game).await.unwrap();
        let path: PathBuf = format!("./{}_relay.json", game.id()).into();
        save_game_relays(&data, path);
    }
}
