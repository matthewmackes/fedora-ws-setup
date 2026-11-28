//! Pre-save "Test connection": resolve, reach and authenticate without
//! mounting or needing root, so the Add wizard can show real results first.

use crate::mount;
use crate::place::{Place, Protocol};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct ProbeStep {
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

fn step(label: &str, ok: bool, detail: impl Into<String>) -> ProbeStep {
    ProbeStep { label: label.into(), ok, detail: detail.into() }
}

/// Arguments for a password-only SSH login that just runs `true`.
pub fn ssh_args(place: &Place) -> Vec<String> {
    let target = if place.username.is_empty() {
        place.host.clone()
    } else {
        format!("{}@{}", place.username, place.host)
    };
    let mut a: Vec<String> = [
        "-o", "PubkeyAuthentication=no",
        "-o", "PreferredAuthentications=password",
        "-o", "NumberOfPasswordPrompts=1",
        "-o", "StrictHostKeyChecking=accept-new",
        "-o", "ConnectTimeout=10",
        "-p",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push(place.port.unwrap_or(22).to_string());
    a.push(target);
    a.push("true".into());
    a
}

/// Run all steps; stops at the first failure.
pub fn run(place: &Place, password: &str) -> Vec<ProbeStep> {
    let mut out = Vec::new();
    let host = place.host.trim();
    if host.is_empty() && place.url.is_empty() {
        out.push(step("Host", false, "Host or URL is required."));
        return out;
    }

    let ip = mount::resolve_ip_pub(host);
    match ip {
        Some(ip) => out.push(step("Resolve host", true, format!("{host} → {ip}"))),
        None => {
            out.push(step("Resolve host", false, format!("Name or service not known: {host}")));
            return out;
        }
    }

    let reachable = mount::place_reachable(place);
    if !reachable {
        out.push(step(
            "Reach host",
            false,
            format!("Host {host} is not reachable on its service port"),
        ));
        return out;
    }
    out.push(step("Reach host", true, "service port is open"));

    match place.protocol {
        Protocol::Ssh => {
            if password.is_empty() {
                out.push(step("Authenticate", true, "no password entered; will use keys or prompt"));
                return out;
            }
            let res = Command::new("sshpass")
                .env("SSHPASS", password)
                .arg("-e")
                .arg("ssh")
                .args(ssh_args(place))
                .stdin(Stdio::null())
                .output();
            match res {
                Ok(o) if o.status.success() => out.push(step("Authenticate", true, "password accepted")),
                Ok(o) => out.push(step(
                    "Authenticate",
                    false,
                    String::from_utf8_lossy(&o.stderr).trim().chars().take(300).collect::<String>(),
                )),
                Err(e) => out.push(step("Authenticate", false, format!("spawn sshpass: {e}"))),
            }
        }
        Protocol::Smb => match mount::smb_list_shares(host, &place.username, password) {
            Ok(shares) => out.push(step("Authenticate", true, format!("{} share(s) visible", shares.len()))),
            Err(e) => out.push(step("Authenticate", false, e.to_string())),
        },
        Protocol::Video | Protocol::Audio => match crate::media::check_login(place, password) {
            Ok(msg) => out.push(step("Sign in", true, msg)),
            Err(e) => out.push(step("Sign in", false, format!("{e:#}"))),
        },
        _ => out.push(step("Authenticate", true, "checked when you connect")),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_args_force_password_and_port() {
        let mut p = Place::new("x", Protocol::Ssh, "10.0.0.2");
        p.username = "mm".into();
        p.port = Some(2200);
        let a = ssh_args(&p);
        assert!(a.contains(&"PubkeyAuthentication=no".to_string()));
        assert!(a.windows(2).any(|w| w == ["-p", "2200"]));
        assert_eq!(a[a.len() - 2], "mm@10.0.0.2");
        assert_eq!(a.last().unwrap(), "true");
    }

    #[test]
    fn empty_host_fails_fast() {
        let p = Place::new("x", Protocol::Ssh, "");
        let steps = run(&p, "");
        assert_eq!(steps.len(), 1);
        assert!(!steps[0].ok);
    }

    #[test]
    fn unresolvable_host_stops_early() {
        let p = Place::new("x", Protocol::Ssh, "no-such-host.invalid");
        let steps = run(&p, "");
        assert!(!steps.last().unwrap().ok);
        assert_eq!(steps.last().unwrap().label, "Resolve host");
    }

    #[test]
    fn closed_port_is_reported() {
        // Port 1 on loopback is closed.
        let mut p = Place::new("x", Protocol::Ssh, "127.0.0.1");
        p.port = Some(1);
        let steps = run(&p, "");
        assert_eq!(steps.last().unwrap().label, "Reach host");
        assert!(!steps.last().unwrap().ok);
    }

    #[test]
    fn open_port_with_non_auth_protocol_passes() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut p = Place::new("x", Protocol::Nfs, "127.0.0.1");
        p.port = Some(l.local_addr().unwrap().port());
        let steps = run(&p, "");
        assert!(steps.iter().all(|s| s.ok), "{steps:?}");
    }
}
