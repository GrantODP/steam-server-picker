use std::{
    collections::HashMap,
    io::{self, ErrorKind},
    path::PathBuf,
};

use crate::{
    Result,
    error::Error,
    firewall::RelayPolicy,
    games::{GameID, SteamGame},
    relay::{GeoRelay, Ping, PingResult, RelayData},
    scanner::{RelayId, fetch_save_game_relays_cache, read_cluster_cache},
};
#[derive(Debug, Default)]
pub struct GameServers {
    game: GameID,
    servers: HashMap<RelayId, GeoRelay>,
    // firewall: FirewallState,
}

impl GameServers {
    pub fn new(game: GameID) -> Self {
        Self {
            game,
            ..Default::default()
        }
    }
    pub fn game(&self) -> GameID {
        self.game.clone()
    }

    //     pub fn block(&mut self, id: RelayId) -> Result<()> {
    //         let servers = self.servers.get(&id);
    //         if let Some(relay) = servers {
    //             let mut policy = RelayPolicy::new(Rule::Deny);
    //             for server in relay.servers() {
    //                 let rule = FirewallRule::deny(server.address());
    //                 policy.add(rule)
    //             }
    //             self.firewall.add_policy(id, policy)?;
    //         } else {
    //         }
    //         Ok(())
    //     }
    //
    //     pub fn allow(&mut self, id: RelayId) -> Result<()> {
    //         let servers = self.servers.get(&id);
    //         if let Some(relay) = servers {
    //             let mut policy = RelayPolicy::new(Rule::Allow);
    //             for server in relay.servers() {
    //                 let rule = FirewallRule::allow(server.address());
    //                 policy.add(rule)
    //             }
    //
    //             self.firewall.add_policy(id, policy)?;
    //         } else {
    //         }
    //
    //         Ok(())
    //     }

    //TODO: Proper result handeling
    pub fn load_clusters(&mut self, mut cache_dir: PathBuf) -> Result<()> {
        cache_dir.push(self.game.cache_name());
        let result = read_cluster_cache(cache_dir);
        match result {
            Ok(map) => {
                self.servers = map
                    .into_iter()
                    .map(|(sname, relay)| (sname, relay.into()))
                    .collect();
            }

            Err(err) => {
                if let Error::Io(io) = err {
                    if io.kind() == ErrorKind::NotFound {
                        return Err(
                        io::Error::new(io.kind(), format!(
                            "Failed to open cache file for game [{}]. Consider fetching data first.\n{}", self.game.name(), io)
                        ).into());
                    }
                }
            }
        }

        Ok(())
    }

    // pub async fn fetch(&self) -> Result<()> {
    //     fetch_save_game_relays_cache(&self.game).await?;
    //     Ok(())
    // }

    // pub fn relay_state(&self, id: &RelayId) -> Option<&Rule> {
    //     self.firewall.relay_state(id)
    // }

    pub async fn refresh_cache(&self) -> Result<()> {
        fetch_save_game_relays_cache(&self.game).await
    }

    pub fn servers_iter(&self) -> impl Iterator<Item = (&RelayId, &GeoRelay)> {
        self.servers.iter()
    }

    pub async fn ping(&self, id: &RelayId) -> PingResult {
        let servers = self.servers.get(id);
        if let Some(relay) = servers {
            return relay.ping().await;
        }
        PingResult::default()
    }

    pub fn relay_ids(&self) -> impl Iterator<Item = &RelayId> {
        self.servers.keys().into_iter()
    }

    pub fn relays(&self) -> Vec<(&RelayId, &GeoRelay)> {
        let mut relays: Vec<(&RelayId, &GeoRelay)> = self.servers.iter().collect();
        relays.sort_by_key(|(id, relay)| &relay.desc);
        relays
    }

    pub fn relay_owned(&self, id: &RelayId) -> Option<RelayData> {
        if let Some(relay) = self.servers.get(id) {
            return Some(relay.relays_owned());
        }

        None
    }

    pub fn get_relay(&self, id: &RelayId) -> Option<&GeoRelay> {
        self.servers.get(id)
    }
}

pub trait PolicyProducer {
    fn build_policy(&self, id: RelayId) -> Option<RelayPolicy>;
}
//
// impl PolicyProducer for ServerPicker {
//     fn build_policy(&self, id: RelayId) -> Option<RelayPolicy> {
//         let servers = self.servers.get(&id);
//         if let Some(relay) = servers {
//             let policy = relay.into();
//             return Some(policy);
//         }
//         None
//     }
// }
