use std::{collections::HashMap, net::IpAddr, time::Duration};

use serde::{Deserialize, Serialize};

use crate::scanner::{self, Pop, Relay};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PingResult {
    pub latency_ms: Duration,
    pub timeout: bool,
}

impl PingResult {
    pub fn new(latency_ms: Duration, timeout: bool) -> Self {
        Self {
            latency_ms,
            timeout,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct GeoRelay {
    pub sname: String,
    pub desc: String,
    pub relays: Vec<Relay>,
    // avg_ping: Duration,
}

impl GeoRelay {
    pub fn servers(&self) -> &Vec<Relay> {
        &self.relays
    }

    pub fn relays_owned(&self) -> RelayData {
        let relays = self.relays.clone();
        RelayData { relays }
    }
}

impl Ping for GeoRelay {
    async fn ping(&self) -> PingResult {
        let mut best_result: Option<PingResult> = None;
        for relay in &self.relays {
            let result = relay.ping().await;
            match best_result {
                None => best_result = Some(result),
                Some(mut prev) => {
                    if !result.timeout {
                        if result.latency_ms < prev.latency_ms {
                            prev = result;
                        }
                    }
                    best_result = Some(prev);
                }
            }
        }
        best_result.unwrap_or_else(|| PingResult::default())
    }
}

impl Ping for Relay {
    async fn ping(&self) -> PingResult {
        let ping_r = ping_address(self.address());
        let ping = match ping_r {
            Some(dur) => PingResult {
                latency_ms: dur,
                timeout: false,
            },
            None => PingResult {
                latency_ms: Duration::from_millis(0),
                timeout: true,
            },
        };
        ping
    }
}
#[derive(Debug, Default)]
pub struct RelayData {
    pub relays: Vec<Relay>,
}

impl Ping for RelayData {
    async fn ping(&self) -> PingResult {
        let mut best_result: Option<PingResult> = None;
        for relay in &self.relays {
            let result = relay.ping().await;
            match best_result {
                None => best_result = Some(result),
                Some(mut prev) => {
                    if !result.timeout {
                        if result.latency_ms < prev.latency_ms {
                            prev = result;
                        }
                    }
                    best_result = Some(prev);
                }
            }
        }
        best_result.unwrap_or_else(|| PingResult::default())
    }
}

fn ping_address(address: IpAddr) -> Option<Duration> {
    match ping::ping(address, Duration::from_secs(1)) {
        Ok(reply) => Some(reply.rtt),
        Err(_) => None,
    }
}

pub trait Ping {
    async fn ping(&self) -> PingResult;
}

pub trait RelayInfo: Ping {
    fn address(&self) -> IpAddr;
}

impl RelayInfo for Relay {
    fn address(&self) -> IpAddr {
        self.ipv4.into()
    }
}

pub struct RelayClusters {
    relay_map: HashMap<String, GeoRelay>,
}

impl From<Pop> for GeoRelay {
    fn from(value: Pop) -> Self {
        Self {
            sname: value.sname,
            desc: value.desc,
            relays: value.relays,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::Path, println};

    use crate::{error::Error, scanner::read_cluster_cache};

    #[tokio::test]
    async fn test_from_pop_with_json() {
        let json_path = Path::new("clusters_test.json");

        // Read from the saved JSON cache
        let pop = read_cluster_cache(&json_path)
            .map_err(Error::from)
            .expect("Failed to read cluster cache");

        println!("ORIGINAL POP LEN: {}", pop.len());

        // Convert Pop to GeoRelay using the From trait
        for (name, p) in pop {
            let geo_relay = GeoRelay::from(p);
            // Verify the conversion worked
            // assert!(!geo_relay.desc.is_empty());
            // assert!(!geo_relay.relays.is_empty());

            // Print some debug info
            println!("GeoRelay Description: {}", geo_relay.desc);
            println!("Number of Relays: {}", geo_relay.relays.len());
        }
        // Test the ping trait on the first relay
        // let first_relay = &geo_relay.relays[0];
        // let ping_result = first_relay.ping().await;
        // println!("First relay ping result: {}", ping_result);
    }
}
