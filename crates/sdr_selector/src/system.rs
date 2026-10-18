use crate::system::RootAcess::{Access, Denied};
use crate::{
    Result,
    error::Error,
    firewall::{RelayPolicy, Rule},
};
use std::{fmt::Write as _, process::Stdio};
use std::{io, path::PathBuf, process::Command};
use std::{io::Write, net::IpAddr};

// pub struct RDenied {
//     reason: String,
// }

pub enum RootAcess {
    Access,
    Denied(io::Error),
}

// #[inline]
// #[cfg(target_os = "linux")]
// pub fn get_root_access() -> RootAcess {
//     let r = Command::new("pkexec").args(&["echo"]).output();
//     match r {
//         Ok(_) => Access,
//         Err(e) => Denied(e),
//     }
// }

pub fn get_cache_dir() -> PathBuf {
    let cache_dir = if cfg!(target_os = "windows") {
        format!(
            r"C:\Users\{}\AppData\Local\SteamServerPicker",
            std::env::var("USERNAME").unwrap_or_else(|_| "Unknown".to_string()),
        )
    } else if cfg!(target_os = "macos") {
        format!(
            "/Users/{}@Library/Application Support/SteamServerPicker",
            std::env::var("USER").unwrap_or_else(|_| "Unknown".to_string()),
        )
    } else {
        format!(
            "/home/{}/.cache/SteamServerPicker",
            std::env::var("USER").unwrap_or_else(|_| "Unknown".to_string()),
        )
    };

    cache_dir.into()
}

#[inline]
#[cfg(target_os = "linux")]
pub fn system_firewall_add_policy(id: u64, policy: &RelayPolicy) -> Result<()> {
    let action = match policy.rule() {
        Rule::Allow => "accept",
        Rule::Deny => "drop",
    };

    let mut script = String::new();
    let policy_name = format!("relay_policy_{}", id);

    script.push_str(&format!("add table inet {policy_name}\n"));
    script.push_str(&format!("delete table inet {policy_name}\n"));
    script.push_str(&format!("table inet {policy_name} {{\n"));
    script.push_str("  chain output {\n");
    script.push_str("    type filter hook output priority 0; policy accept;\n");

    for relay in policy.rules() {
        let fam = match relay.address {
            IpAddr::V4(_) => "ip",
            IpAddr::V6(_) => "ip6",
        };
        writeln!(script, "    {fam} saddr {} {action}", relay.address);
        writeln!(script, "    {fam} daddr {} {action}", relay.address);
    }

    script.push_str("  }\n}\n");

    run_nft(&script)
}
#[cfg(target_os = "linux")]
fn run_nft(script: &str) -> Result<()> {
    let mut child = Command::new("pkexec")
        .args(["nft", "-f", "-"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(script.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let output = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(Error::FirewallScriptFailed(output));
    }
    Ok(())
}

#[inline]
#[cfg(target_os = "linux")]
pub fn system_firewall_remove_policy(id: u64) -> Result<()> {
    let policy_name = format!("relay_policy_{}", id);
    let script = format!("add table inet {policy_name}\ndelete table inet {policy_name}\n");
    run_nft(&script)
}

#[cfg(target_os = "windows")]
pub fn system_firewall_add_policy(policy: &RelayPolicy) -> Result<()> {
    use std::process::Command;

    let mut script = String::new();
    // Convert policy action (e.g., "allow" or "block") to TitleCase for Windows
    let rule = policy.rule();
    let action = match rule {
        Rule::Allow => "Allow",
        Rule::Deny => "Block",
    };

    for relay in policy.rules() {
        let addr = relay.address.to_string();
        // Generate a unique rule name using the address to avoid collisions
        let name_in = format!("Relay_In_{}_{}", action, addr.replace('.', "_"));
        let name_out = format!("Relay_Out_{}_{}", action, addr.replace('.', "_"));

        // Outbound Rule
        script.push_str(&format!(
            "New-NetFirewallRule -DisplayName '{}' -Direction Outbound -Action {} -RemoteAddress {}; ",
            name_out, action, addr
        ));
        // Inbound Rule
        script.push_str(&format!(
            "New-NetFirewallRule -DisplayName '{}' -Direction Inbound -Action {} -RemoteAddress {}; ",
            name_in, action, addr
        ));
    }

    // Execute PowerShell via UAC prompt ("runas")
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Start-Process powershell -ArgumentList '-NoProfile -Command \"{}\"' -Verb RunAs",
                script
            ),
        ])
        .status()?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn system_firewall_remove_policy(policy: &RelayPolicy) -> Result<()> {
    use std::process::Command;

    let mut script = String::new();
    let rule = policy.rule();
    let action = match rule {
        Rule::Allow => "Allow",
        Rule::Deny => "Block",
    };

    for relay in policy.rules() {
        let addr = relay.address.to_string();
        let name_in = format!("Relay_In_{}_{}", action, addr.replace('.', "_"));
        let name_out = format!("Relay_Out_{}_{}", action, addr.replace('.', "_"));

        // Remove rules (silencing errors with -ErrorAction SilentlyContinue if rule doesn't exist)
        script.push_str(&format!(
            "Remove-NetFirewallRule -DisplayName '{}' -ErrorAction SilentlyContinue; ",
            name_out
        ));
        script.push_str(&format!(
            "Remove-NetFirewallRule -DisplayName '{}' -ErrorAction SilentlyContinue; ",
            name_in
        ));
    }

    // Execute PowerShell via UAC prompt ("runas")
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Start-Process powershell -ArgumentList '-NoProfile -Command \"{}\"' -Verb RunAs",
                script
            ),
        ])
        .status()?;

    Ok(())
}
