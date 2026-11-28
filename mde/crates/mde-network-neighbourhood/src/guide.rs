//! Logic behind the guided "Add place" flow: what each connection type is, which
//! standard port it uses, how to clean up whatever the user typed, and how to turn
//! the answers into a `Place`. No GUI here so it can be unit-tested.

use crate::place::{DiscoveredHost, MountPolicy, Place, Protocol};

/// Connection types in the order they are offered.
pub const KINDS: [Protocol; 6] = [
    Protocol::Smb,
    Protocol::Ssh,
    Protocol::Nfs,
    Protocol::WebDav,
    Protocol::Audio,
    Protocol::Video,
];

pub fn title(p: Protocol) -> &'static str {
    match p {
        Protocol::Smb => "Windows or Samba shared folder",
        Protocol::Ssh => "Another computer (SSH)",
        Protocol::Nfs => "NAS or Linux share (NFS)",
        Protocol::WebDav => "WebDAV or cloud folder",
        Protocol::Audio => "Music server",
        Protocol::Video => "Video server",
    }
}

pub fn blurb(p: Protocol) -> &'static str {
    match p {
        Protocol::Smb => "Shared folders on Windows PCs, Macs, NAS boxes and Samba",
        Protocol::Ssh => "Browse files on any machine you can log in to over SSH",
        Protocol::Nfs => "Exports from a NAS or Linux server",
        Protocol::WebDav => "Nextcloud, ownCloud and other WebDAV services",
        Protocol::Audio => "Airsonic, Navidrome and other Subsonic servers",
        Protocol::Video => "Jellyfin movies and shows",
    }
}

/// Heading for the "where" step.
pub fn where_heading(p: Protocol) -> &'static str {
    match p {
        Protocol::Smb => "Which computer has the shared folder?",
        Protocol::Ssh => "Which computer do you want to reach?",
        Protocol::Nfs => "Which server has the share?",
        Protocol::WebDav => "What is the server address?",
        Protocol::Audio => "Where is your music server?",
        Protocol::Video => "Where is your Jellyfin server?",
    }
}

/// How a discovered host is listed: just the address when its name only repeats it.
pub fn host_label(h: &DiscoveredHost) -> String {
    if h.name.is_empty() || h.name.contains(&h.host) {
        h.host.clone()
    } else {
        format!("{}  ·  {}", h.name, h.host)
    }
}

/// Needs a username/password step. NFS trusts the machine, so it has none.
pub fn needs_login(p: Protocol) -> bool {
    !matches!(p, Protocol::Nfs)
}

/// What the "What to open" step asks for, if anything: (heading, hint, example).
pub fn target_prompt(p: Protocol) -> Option<(&'static str, &'static str, &'static str)> {
    match p {
        Protocol::Smb => Some(("Which shared folder?", "Pick one below, or type its name.", "media")),
        Protocol::Nfs => Some(("Which export?", "The folder the server shares.", "/export/media")),
        Protocol::WebDav => Some(("Which folder?", "The path after the address, if any.", "remote.php/dav/files/me")),
        Protocol::Ssh => Some(("Which folder?", "Leave empty to open your home folder.", "/srv/data")),
        Protocol::Audio | Protocol::Video => None,
    }
}

/// A target is required only where a blank would make no sense.
pub fn target_required(p: Protocol) -> bool {
    matches!(p, Protocol::Smb | Protocol::Nfs)
}

/// What a user typed in the host box, split up.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Parsed {
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
    pub user: String,
}

/// Accepts a bare host, `host:port`, `\\host\share`, `smb://user@host/share`, `http://host:8096`.
/// Anything beyond the host is returned separately so nothing the user typed is lost.
pub fn parse_host_input(input: &str) -> Parsed {
    let mut s = input.trim();
    if let Some(i) = s.find("://") {
        s = &s[i + 3..];
    }
    s = s.trim_start_matches(['\\', '/']);

    let (authority, path) = match s.find(['/', '\\']) {
        Some(i) => (&s[..i], s[i + 1..].trim_matches(['/', '\\'])),
        None => (s, ""),
    };
    let (user, hostport) = match authority.rsplit_once('@') {
        Some((u, h)) => (u, h),
        None => ("", authority),
    };
    // `host:port`, but leave bare IPv6 alone.
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') && p.parse::<u16>().is_ok() => (h, p.parse().ok()),
        _ => (hostport, None),
    };
    Parsed {
        host: host.trim().to_string(),
        port,
        path: path.replace('\\', "/"),
        user: user.to_string(),
    }
}

/// A friendly default name, e.g. "nas (SMB)".
pub fn suggest_name(p: Protocol, host: &str) -> String {
    format!("{host} ({})", p.group_label())
}

/// Hosts already seen on the network that offer this kind of service,
/// reachable ones first, then by name. Duplicates by host collapse.
pub fn host_candidates(found: &[DiscoveredHost], p: Protocol) -> Vec<DiscoveredHost> {
    let mut v: Vec<DiscoveredHost> = found.iter().filter(|h| h.protocol == p).cloned().collect();
    v.sort_by(|a, b| {
        b.reachable
            .cmp(&a.reachable)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    v.dedup_by(|a, b| a.host == b.host);
    v
}

/// Everything the flow collected.
#[derive(Debug, Clone, Default)]
pub struct Draft {
    pub protocol: Option<Protocol>,
    pub host: String,
    /// Only set when the user chose a non-standard port.
    pub port: Option<u16>,
    pub user: String,
    pub password: String,
    pub guest: bool,
    pub target: String,
    pub name: String,
}

pub fn build_place(d: &Draft) -> Option<Place> {
    let protocol = d.protocol?;
    let name = if d.name.trim().is_empty() {
        suggest_name(protocol, &d.host)
    } else {
        d.name.trim().to_string()
    };
    let mut p = Place::new(name, protocol, d.host.trim());
    p.port = d.port; // None = the standard port for this protocol
    p.path = d.target.trim().to_string();
    p.username = if d.guest { String::new() } else { d.user.trim().to_string() };
    p.mount_policy = MountPolicy::Automount;
    p.sync_automount_flag();
    Some(p)
}

/// The steps for a protocol, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Kind,
    Host,
    Login,
    Target,
    Check,
}

pub fn steps_for(p: Option<Protocol>) -> Vec<Step> {
    let Some(p) = p else {
        return vec![Step::Kind, Step::Host, Step::Check];
    };
    let mut v = vec![Step::Kind, Step::Host];
    if needs_login(p) {
        v.push(Step::Login);
    }
    if target_prompt(p).is_some() {
        v.push(Step::Target);
    }
    v.push(Step::Check);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str, h: &str, p: Protocol, up: bool) -> DiscoveredHost {
        DiscoveredHost {
            name: name.into(),
            host: h.into(),
            protocol: p,
            port: None,
            path: String::new(),
            url: String::new(),
            reachable: up,
            source: "test".into(),
            last_seen: 0,
        }
    }

    #[test]
    fn parses_plain_and_decorated_input() {
        assert_eq!(parse_host_input("nas").host, "nas");
        assert_eq!(parse_host_input("  nas.local ").host, "nas.local");
        let p = parse_host_input("192.168.1.5:2222");
        assert_eq!((p.host.as_str(), p.port), ("192.168.1.5", Some(2222)));
        let p = parse_host_input("smb://me@nas/media/tv");
        assert_eq!((p.host.as_str(), p.user.as_str(), p.path.as_str()), ("nas", "me", "media/tv"));
        let p = parse_host_input(r"\\fileserver\docs");
        assert_eq!((p.host.as_str(), p.path.as_str()), ("fileserver", "docs"));
        let p = parse_host_input("http://media:8096/");
        assert_eq!((p.host.as_str(), p.port), ("media", Some(8096)));
    }

    #[test]
    fn ipv6_and_junk_ports_are_left_alone() {
        assert_eq!(parse_host_input("fe80::1").host, "fe80::1");
        assert_eq!(parse_host_input("nas:abc").host, "nas:abc");
    }

    #[test]
    fn candidates_filter_sort_and_dedupe() {
        let found = vec![
            host("zeta", "10.0.0.9", Protocol::Smb, false),
            host("alpha", "10.0.0.2", Protocol::Smb, true),
            host("alpha-again", "10.0.0.2", Protocol::Smb, true),
            host("ssh-box", "10.0.0.3", Protocol::Ssh, true),
        ];
        let v = host_candidates(&found, Protocol::Smb);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].host, "10.0.0.2");
        assert!(v[0].reachable && !v[1].reachable);
    }

    #[test]
    fn standard_port_is_the_default() {
        let d = Draft {
            protocol: Some(Protocol::Ssh),
            host: "box".into(),
            user: "mm".into(),
            ..Default::default()
        };
        let p = build_place(&d).unwrap();
        assert_eq!(p.port, None);
        assert_eq!(p.username, "mm");
        assert_eq!(p.name, "box (SSH)");
        assert_eq!(p.mount_policy, MountPolicy::Automount);
    }

    #[test]
    fn guest_clears_username_and_custom_port_is_kept() {
        let d = Draft {
            protocol: Some(Protocol::Smb),
            host: "nas".into(),
            user: "ignored".into(),
            guest: true,
            port: Some(4450),
            target: " media ".into(),
            ..Default::default()
        };
        let p = build_place(&d).unwrap();
        assert!(p.username.is_empty());
        assert_eq!((p.port, p.path.as_str()), (Some(4450), "media"));
    }

    #[test]
    fn step_lists_match_what_each_type_needs() {
        use Step::*;
        assert_eq!(steps_for(Some(Protocol::Smb)), [Kind, Host, Login, Target, Check]);
        assert_eq!(steps_for(Some(Protocol::Nfs)), [Kind, Host, Target, Check]);
        assert_eq!(steps_for(Some(Protocol::Video)), [Kind, Host, Login, Check]);
        assert_eq!(steps_for(Some(Protocol::Ssh)), [Kind, Host, Login, Target, Check]);
    }

    #[test]
    fn host_label_avoids_repeating_the_address() {
        assert_eq!(host_label(&host("ssh:10.0.0.2", "10.0.0.2", Protocol::Ssh, true)), "10.0.0.2");
        assert_eq!(host_label(&host("NAS", "10.0.0.3", Protocol::Smb, true)), "NAS  ·  10.0.0.3");
        assert_eq!(host_label(&host("", "10.0.0.4", Protocol::Smb, true)), "10.0.0.4");
    }

    #[test]
    fn every_kind_has_text() {
        for p in KINDS {
            assert!(!title(p).is_empty() && !blurb(p).is_empty() && !where_heading(p).is_empty());
        }
    }
}
