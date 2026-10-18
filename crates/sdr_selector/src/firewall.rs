use serde::{Deserialize, Serialize};

use crate::Result;
use crate::error::Error;
use crate::relay::{GeoRelay, RelayInfo};
use crate::scanner::RelayId;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::path::Path;
use std::println;
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Rule {
    #[default]
    Allow,
    Deny,
}

impl Rule {
    pub fn to_string(&self) -> &'static str {
        match self {
            Rule::Allow => "allow",
            Rule::Deny => "deny",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirewallRule {
    pub address: IpAddr,
    pub rule: Rule,
}

impl Hash for FirewallRule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ActivePolicy {
    pub id: RelayId,
    pub policy: RelayPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RelayPolicy {
    policy: Rule,
    firewall_rules: HashSet<FirewallRule>,
}

impl RelayPolicy {
    pub fn new(policy: Rule) -> Self {
        let firewall_rules = HashSet::new();
        Self {
            policy,
            firewall_rules,
        }
    }

    pub fn change(&mut self, rule: Rule) {
        self.policy = rule
    }

    pub fn apply(self, id: RelayId) -> Result<ActivePolicy> {
        if self.firewall_rules.is_empty() {
            println!("EMPTY RULE SET, NO IP's");
            return Err(Error::FirewallEmptyPolicy);
        }

        add_policy(&id, &self)?;
        let policy = ActivePolicy {
            id: id,
            policy: self,
        };
        Ok(policy)
    }

    pub fn add(&mut self, rule: FirewallRule) {
        assert!(rule.rule == self.policy);
        self.firewall_rules.insert(rule);
    }

    pub fn remove(&mut self, rule: FirewallRule) -> Result<FirewallRule> {
        if self.firewall_rules.remove(&rule) {
            remove_rule(&rule)?;
            return Ok(rule);
        }
        Err(Error::UnknownServerID)
    }

    pub fn rule(&self) -> &Rule {
        &self.policy
    }
    pub fn rules<'a>(&'a self) -> impl Iterator<Item = &FirewallRule> {
        self.firewall_rules.iter()
    }
}

impl ActivePolicy {
    pub fn suspend(self) -> Result<RelayPolicy> {
        remove_policy(&self.id)?;
        Ok(self.policy)
    }
}

impl FirewallRule {
    pub fn new(address: IpAddr, rule: Rule) -> Self {
        Self { address, rule }
    }

    pub fn allow(addr: IpAddr) -> Self {
        Self {
            address: addr,
            rule: Rule::Allow,
        }
    }

    pub fn deny(addr: IpAddr) -> Self {
        Self {
            address: addr,
            rule: Rule::Deny,
        }
    }
}

#[cfg(target_os = "linux")]
pub fn add_rule(rule: &FirewallRule) -> Result<()> {
    // return Ok(());
    let ufw_cmd = "ufw";
    let ip = rule.address.to_string();
    let action = rule.rule.to_string();

    let status = Command::new("pkexec")
        .args(&[ufw_cmd, action, "from", &ip, "to", "any"])
        .status();
    if let Err(_) = status {
        use crate::error::Error;
        return Err(Error::Firewall);
    }

    Ok(())
}
#[cfg(target_os = "linux")]
pub fn remove_rule(rule: &FirewallRule) -> Result<()> {
    // return Ok(());
    let ufw_cmd = "ufw";
    let ip = rule.address.to_string();
    let action = rule.rule.to_string();

    let status = Command::new("pkexec")
        .args(&[ufw_cmd, "delete", action, "from", &ip, "to", "any"])
        .status();

    if let Err(_) = status {
        use crate::error::Error;
        return Err(Error::Firewall);
    }
    Ok(())
}
pub fn add_policy(id: &RelayId, policy: &RelayPolicy) -> Result<()> {
    use crate::system::system_firewall_add_policy;
    system_firewall_add_policy(id.into(), policy)
}

pub fn remove_policy(id: &RelayId) -> Result<()> {
    use crate::system::system_firewall_remove_policy;
    system_firewall_remove_policy(id.into())
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FirewallState {
    pub active_policy: HashMap<RelayId, ActivePolicy>,
    pub cluster_state_cache: HashMap<RelayId, Rule>,
}

impl FirewallState {
    pub fn add_policy(&mut self, id: RelayId, policy: RelayPolicy) -> Result<()> {
        let active = policy.apply(id)?;
        println!("{:?}", active);
        self.active_policy.insert(id, active);
        Ok(())
    }

    pub fn remove_policy(&mut self, id: RelayId) -> Result<RelayPolicy> {
        if let Some(policy) = self.active_policy.remove(&id) {
            let rpolicy = policy.suspend()?;
            return Ok(rpolicy);
        } else {
            return Err(Error::UnknownServerID);
        }
    }

    pub fn apply_rule(&mut self, id: RelayId, rule: Rule) -> Result<()> {
        if let Some(policy) = self.active_policy.remove(&id) {
            let mut old = policy.suspend()?;
            old.change(rule);
            self.add_policy(id, old)
        } else {
            return Err(Error::UnknownServerID);
        }
    }

    pub fn relay_state(&self, id: &RelayId) -> Option<&Rule> {
        if let Some(active) = self.active_policy.get(&id) {
            return Some(active.policy.rule());
        }
        None
    }

    pub fn contains(&self, id: &RelayId) -> bool {
        self.active_policy.contains_key(id)
    }
}

pub trait Storable {
    fn load<P: AsRef<Path>>(&mut self, dir: P) -> Result<()>;

    fn store<P: AsRef<Path>>(&self, dir: P) -> Result<()>;
}

impl Storable for FirewallState {
    fn load<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let path = dir.as_ref();
        let file_path = path.join("firewall.json");
        if !file_path.exists() {
            let error =
                std::io::Error::new(std::io::ErrorKind::NotFound, "firewall.json not found");
            return Err(error.into());
        }

        let content = std::fs::read_to_string(&file_path)?;
        let state: FirewallState = serde_json::from_str(&content)?;
        *self = state;
        Ok(())
    }

    fn store<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        let path = dir.as_ref();
        std::fs::create_dir_all(path)?;
        let file_path = path.join("firewall.json");
        let text = serde_json::to_string(self)?;
        std::fs::write(&file_path, &text)?;
        Ok(())
    }
}

impl From<&GeoRelay> for RelayPolicy {
    fn from(value: &GeoRelay) -> Self {
        let rule = Rule::Allow;
        RelayPolicy {
            policy: rule,
            firewall_rules: value
                .relays
                .iter()
                .map(|r| FirewallRule::allow(r.address()))
                .collect(),
        }
    }
}
