use thiserror::Error;
use tokio::io;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network Error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("Json Parsing error")]
    Parsing(#[from] serde_json::Error),

    #[error("Firewall Error")]
    Firewall,

    #[error("Unable to access firewall")]
    FirewallUnavailable,

    #[error("Policy in firewall has no IP's to apply to")]
    FirewallEmptyPolicy,

    #[error("Provided server is not known or registered with server picker")]
    UnknownServerID,

    #[error("Ip address for firewall rule is not known")]
    RuleIpAddressUnknown,

    #[error("Firewall rule not found")]
    RuleNotFound,

    #[error("{0}")]
    FirewallScriptFailed(String),
}
