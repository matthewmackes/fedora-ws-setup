use crate::config::{delete_credentials, write_credentials, Config};
use crate::discovery;
use crate::mount;
use crate::place::{DiscoveredHost, Place, Protocol};
use crate::{DBUS_NAME, DBUS_PATH};
use anyhow::{Context, Result};
use serde_json::json;
use std::time::Duration;
use zbus::{connection, interface, Connection};

pub struct NeighbourhoodService;

impl NeighbourhoodService {
    pub fn new() -> Self {
        Self
    }
}

#[interface(name = "org.mde.NetworkNeighbourhood1")]
impl NeighbourhoodService {
    async fn list_places(&self) -> zbus::fdo::Result<String> {
        let cfg = Config::load().map_err(to_fdo)?;
        serde_json::to_string(&cfg.places).map_err(to_fdo)
    }

    async fn get_settings(&self) -> zbus::fdo::Result<String> {
        let cfg = Config::load().map_err(to_fdo)?;
        serde_json::to_string(&cfg.settings).map_err(to_fdo)
    }

    async fn set_settings(&self, settings_json: String) -> zbus::fdo::Result<()> {
        let settings: crate::config::Settings =
            serde_json::from_str(&settings_json).map_err(to_fdo)?;
        let mut cfg = Config::load().map_err(to_fdo)?;
        cfg.settings = settings;
        cfg.save().map_err(to_fdo)?;
        Ok(())
    }

    async fn upsert_place(&self, place_json: String, password: String) -> zbus::fdo::Result<String> {
        let mut place: Place = serde_json::from_str(&place_json).map_err(to_fdo)?;
        if place.id.is_empty() {
            place = Place::new(place.name.clone(), place.protocol, place.host.clone());
        }
        place.sync_automount_flag();
        if place.credential_scope == crate::place::CredentialScope::User
            && !place.credential_file.starts_with('/')
        {
            let path = crate::config::user_creds_dir().join(format!("{}.cred", place.id));
            place.credential_file = path.to_string_lossy().to_string();
        }
        if !password.is_empty() {
            write_credentials(&place, &password).map_err(to_fdo)?;
            place.last_error.clear();
        }
        let mut cfg = Config::load().map_err(to_fdo)?;
        cfg.upsert(place.clone());
        cfg.save().map_err(to_fdo)?;
        mount::write_systemd_units(&place).map_err(to_fdo)?;
        let _ = mount::daemon_reload();
        if place.automount {
            let _ = mount::enable_place(&place);
        }
        Ok(place.id)
    }

    async fn remove_place(&self, id: String) -> zbus::fdo::Result<()> {
        let mut cfg = Config::load().map_err(to_fdo)?;
        if let Some(place) = cfg.remove(&id) {
            let _ = mount::unmount_now(&place);
            let _ = mount::remove_systemd_units(&place);
            let _ = mount::daemon_reload();
            let _ = delete_credentials(&place);
            cfg.save().map_err(to_fdo)?;
        }
        Ok(())
    }

    async fn mount_place(&self, id: String) -> zbus::fdo::Result<String> {
        let cfg = Config::load().map_err(to_fdo)?;
        let place = cfg
            .find(&id)
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("unknown place {id}")))?;
        let place_id = place.id.clone();
        let (result, log) = tokio::task::spawn_blocking(move || mount::mount_now_with_log(&place))
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("mount task join: {e}")))?;
        let mut cfg = Config::load().map_err(to_fdo)?;
        if let Some(p) = cfg.find_mut(&place_id) {
            p.last_error = match &result {
                Ok(()) => String::new(),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("cancelled") {
                        String::new()
                    } else {
                        msg
                    }
                }
            };
            let _ = cfg.save();
        }
        // Always return the log; encode failure in connect progress + last_error.
        if let Err(e) = &result {
            let msg = e.to_string();
            if msg.contains("cancelled") {
                return Ok(log.join("\n"));
            }
            return Err(to_fdo(e));
        }
        Ok(log.join("\n"))
    }

    async fn get_connect_status(&self, id: String) -> zbus::fdo::Result<String> {
        let progress = crate::connect::ConnectProgress::load(&id);
        serde_json::to_string(&progress).map_err(to_fdo)
    }

    async fn cancel_connect(&self, id: String) -> zbus::fdo::Result<()> {
        let cfg = Config::load().map_err(to_fdo)?;
        let place = cfg
            .find(&id)
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("unknown place {id}")))?;
        tokio::task::spawn_blocking(move || mount::cancel_connect(&place))
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .map_err(to_fdo)
    }

    async fn open_mount_logs(&self, id: String) -> zbus::fdo::Result<String> {
        let cfg = Config::load().map_err(to_fdo)?;
        let place = cfg
            .find(&id)
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("unknown place {id}")))?;
        let unit = format!(
            "{}.mount",
            mount::systemd_path_unit(&place.mount_point())
        );
        let out = tokio::task::spawn_blocking(move || {
            std::process::Command::new("journalctl")
                .args(["-u", &unit, "-n", "80", "--no-pager"])
                .output()
        })
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    async fn unmount_place(&self, id: String) -> zbus::fdo::Result<()> {
        let cfg = Config::load().map_err(to_fdo)?;
        let place = cfg
            .find(&id)
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("unknown place {id}")))?;
        tokio::task::spawn_blocking(move || mount::unmount_now(&place))
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .map_err(to_fdo)
    }

    async fn disable_automount(&self, id: String) -> zbus::fdo::Result<()> {
        let cfg = Config::load().map_err(to_fdo)?;
        let place = cfg
            .find(&id)
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("unknown place {id}")))?;
        tokio::task::spawn_blocking(move || mount::disable_automount(&place))
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .map_err(to_fdo)
    }

    /// Return the background discovery cache (fast — does not scan).
    async fn list_discovered(&self) -> zbus::fdo::Result<String> {
        let cache = discovery::cache::DiscoveryCache::load();
        serde_json::to_string(&cache).map_err(to_fdo)
    }

    /// Force a discovery pass, update the cache, return hosts JSON array.
    async fn discover(&self) -> zbus::fdo::Result<String> {
        let cfg = Config::load().map_err(to_fdo)?;
        let settings = cfg.settings.clone();
        let cache = tokio::task::spawn_blocking(move || {
            discovery::cache::run_scan_and_store(&settings)
        })
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
        .map_err(to_fdo)?;
        serde_json::to_string(&cache.hosts).map_err(to_fdo)
    }

    async fn get_discovery_status(&self) -> zbus::fdo::Result<String> {
        let cache = discovery::cache::DiscoveryCache::load();
        Ok(json!({
            "updated_at": cache.updated_at,
            "age_secs": cache.age_secs(),
            "scanning": cache.scanning || discovery::cache::is_scanning(),
            "host_count": cache.hosts.len(),
            "last_error": cache.last_error,
            "last_status": cache.last_status,
            "background": Config::load().map(|c| c.settings.discovery_background).unwrap_or(true),
            "interval_sec": Config::load().map(|c| c.settings.discovery_interval_sec).unwrap_or(300),
        })
        .to_string())
    }

    async fn list_smb_shares(
        &self,
        host: String,
        username: String,
        password: String,
    ) -> zbus::fdo::Result<String> {
        let shares = tokio::task::spawn_blocking(move || {
            mount::smb_list_shares(&host, &username, &password)
        })
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
        .map_err(to_fdo)?;
        serde_json::to_string(&shares).map_err(to_fdo)
    }

    async fn place_status(&self, id: String) -> zbus::fdo::Result<String> {
        let cfg = Config::load().map_err(to_fdo)?;
        let place = cfg
            .find(&id)
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::Failed(format!("unknown place {id}")))?;
        let mounted = mount::is_mounted(&place.mount_point());
        let reachable = mount::place_reachable(&place);
        Ok(json!({
            "id": place.id,
            "mounted": mounted,
            "reachable": reachable,
            "mount_point": place.mount_point(),
            "drive_letter": place.drive_symlink_path(),
            "last_error": place.last_error,
        })
        .to_string())
    }

    async fn parse_uri(&self, uri: String) -> zbus::fdo::Result<String> {
        let place = parse_network_uri(&uri)
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("cannot parse URI: {uri}")))?;
        serde_json::to_string(&place).map_err(to_fdo)
    }

    async fn begin_admin_session(&self) -> zbus::fdo::Result<()> {
        mount::ensure_admin_session().map_err(to_fdo)
    }

    async fn daemon_ping(&self) -> zbus::fdo::Result<String> {
        Ok("ok".into())
    }
}

fn to_fdo<E: std::fmt::Display>(e: E) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(e.to_string())
}

pub fn parse_network_uri(uri: &str) -> Option<Place> {
    let uri = uri.trim();
    if uri.is_empty() {
        return None;
    }

    if let Some(rest) = uri.strip_prefix("\\\\").or_else(|| uri.strip_prefix("//")) {
        let parts: Vec<_> = rest.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return None;
        }
        let mut p = Place::new(parts[0], Protocol::Smb, parts[0]);
        if parts.len() > 1 {
            p.path = parts[1].to_string();
            p.name = format!("{}/{}", parts[0], parts[1]);
        }
        return Some(p);
    }

    let parsed = url::Url::parse(uri).ok()?;
    let scheme = parsed.scheme();
    let host = parsed.host_str()?.to_string();
    let port = parsed.port();
    let path = parsed.path().trim_start_matches('/').to_string();

    let protocol = Protocol::from_uri_scheme(scheme).or_else(|| {
        if scheme == "http" || scheme == "https" {
            match port.unwrap_or(if scheme == "https" { 443 } else { 80 }) {
                8096 | 8920 => Some(Protocol::Video),
                4040 => Some(Protocol::Audio),
                _ if path.to_ascii_lowercase().contains("jellyfin") => Some(Protocol::Video),
                _ if path.to_ascii_lowercase().contains("airsonic")
                    || path.to_ascii_lowercase().contains("navidrome") =>
                {
                    Some(Protocol::Audio)
                }
                _ => Some(Protocol::WebDav),
            }
        } else {
            None
        }
    })?;

    let mut place = Place::new(host.clone(), protocol, host);
    place.port = port;
    place.path = path;
    place.url = if matches!(protocol, Protocol::Video | Protocol::Audio | Protocol::WebDav) {
        uri.to_string()
    } else {
        String::new()
    };
    let user = parsed.username();
    if !user.is_empty() {
        place.username = user.to_string();
    }
    place.name = match protocol {
        Protocol::Video => format!("Video — {}", place.host),
        Protocol::Audio => format!("Audio — {}", place.host),
        _ => {
            if place.path.is_empty() {
                place.host.clone()
            } else {
                format!("{}/{}", place.host, place.path)
            }
        }
    };
    Some(place)
}

pub async fn serve_system() -> Result<Connection> {
    let service = NeighbourhoodService::new();
    let conn = connection::Builder::system()?
        .name(DBUS_NAME)?
        .serve_at(DBUS_PATH, service)?
        .build()
        .await?;
    Ok(conn)
}

fn method_timeout_secs(method: &str) -> u64 {
    match method {
        // Keep the GUI responsive — never sit on List/Ping for the full mount window.
        "ListPlaces" | "GetSettings" | "DaemonPing" | "PlaceStatus" | "ParseUri"
        | "ListDiscovered" | "GetDiscoveryStatus" | "GetConnectStatus" => 8,
        "BeginAdminSession" | "SetSettings" | "RemovePlace" | "UnmountPlace"
        | "DisableAutomount" | "UpsertPlace" | "CancelConnect" | "OpenMountLogs" => 30,
        "MountPlace" | "Discover" | "ListSmbShares" => 90,
        _ => 30,
    }
}

/// Sync D-Bus client via zbus (never puts secrets on process argv / `/proc`).
fn with_proxy<F, T>(method: &str, f: F) -> Result<T>
where
    F: FnOnce(&zbus::blocking::Proxy<'_>) -> zbus::Result<T>,
{
    let timeout = Duration::from_secs(method_timeout_secs(method));
    let conn = zbus::blocking::connection::Builder::system()
        .context("system bus")?
        .method_timeout(timeout)
        .build()
        .context("connect system bus")?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        DBUS_NAME,
        DBUS_PATH,
        "org.mde.NetworkNeighbourhood1",
    )
    .context("build neighbourhood proxy")?;
    f(&proxy).map_err(|e| anyhow::anyhow!("dbus {method}: {e}"))
}

pub fn call_sync(method: &str, body: &str) -> Result<String> {
    match method {
        "ListPlaces" | "GetSettings" | "Discover" | "ListDiscovered" | "GetDiscoveryStatus"
        | "BeginAdminSession" | "DaemonPing" => {
            if method == "BeginAdminSession" {
                with_proxy(method, |p| p.call::<_, _, ()>(method, &()))?;
                Ok(String::new())
            } else {
                with_proxy(method, |p| p.call::<_, _, String>(method, &()))
            }
        }
        "UpsertPlace" => {
            let (place_json, password) = body.split_once("\n\n").unwrap_or((body, ""));
            with_proxy(method, |p| {
                p.call::<_, _, String>(method, &(place_json, password))
            })
        }
        "SetSettings" | "RemovePlace" | "MountPlace" | "UnmountPlace" | "DisableAutomount"
        | "PlaceStatus" | "ParseUri" | "GetConnectStatus" | "CancelConnect" | "OpenMountLogs" => {
            if matches!(
                method,
                "SetSettings" | "RemovePlace" | "UnmountPlace" | "DisableAutomount"
                    | "CancelConnect"
            ) {
                with_proxy(method, |p| {
                    p.call::<_, _, ()>(method, &(body,))?;
                    Ok(String::new())
                })
            } else {
                with_proxy(method, |p| p.call::<_, _, String>(method, &(body,)))
            }
        }
        "ListSmbShares" => {
            let parts: Vec<&str> = body.splitn(3, '\n').collect();
            let host = parts.first().copied().unwrap_or("");
            let user = parts.get(1).copied().unwrap_or("");
            let pass = parts.get(2).copied().unwrap_or("");
            with_proxy(method, |p| {
                p.call::<_, _, String>(method, &(host, user, pass))
            })
        }
        _ => anyhow::bail!("unknown dbus method {method}"),
    }
}

pub fn daemon_alive() -> bool {
    call_sync("DaemonPing", "").map(|s| s.contains("ok")).unwrap_or(false)
}

pub fn list_places_sync() -> Result<Vec<Place>> {
    match call_sync("ListPlaces", "") {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        Err(_) => Ok(Config::load().unwrap_or_default().places),
    }
}

pub fn get_settings_sync() -> Result<crate::config::Settings> {
    match call_sync("GetSettings", "") {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        Err(_) => Ok(Config::load().unwrap_or_default().settings),
    }
}

pub fn set_settings_sync(settings: &crate::config::Settings) -> Result<()> {
    let json = serde_json::to_string(settings)?;
    match call_sync("SetSettings", &json) {
        Ok(_) => Ok(()),
        Err(_) => {
            let mut cfg = Config::load().unwrap_or_default();
            cfg.settings = settings.clone();
            cfg.save()?;
            Ok(())
        }
    }
}

pub fn discover_sync() -> Result<Vec<DiscoveredHost>> {
    match call_sync("Discover", "") {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        Err(_) => {
            let cfg = Config::load().unwrap_or_default();
            Ok(discovery::cache::run_scan_and_store(&cfg.settings)
                .map(|c| c.hosts)
                .unwrap_or_else(|_| {
                    discovery::discover_all(&cfg.settings).unwrap_or_default()
                }))
        }
    }
}

pub fn list_discovered_sync() -> Result<discovery::cache::DiscoveryCache> {
    match call_sync("ListDiscovered", "") {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        Err(_) => Ok(discovery::cache::DiscoveryCache::load()),
    }
}

pub fn get_discovery_status_sync() -> Result<serde_json::Value> {
    match call_sync("GetDiscoveryStatus", "") {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or(json!({}))),
        Err(_) => {
            let cache = discovery::cache::DiscoveryCache::load();
            Ok(json!({
                "updated_at": cache.updated_at,
                "age_secs": cache.age_secs(),
                "scanning": cache.scanning,
                "host_count": cache.hosts.len(),
                "last_error": cache.last_error,
                "last_status": cache.last_status,
            }))
        }
    }
}

pub fn upsert_place_sync(place: &Place, password: &str) -> Result<String> {
    let mut place = place.clone();
    place.sync_automount_flag();
    let json = serde_json::to_string(&place)?;
    match call_sync("UpsertPlace", &format!("{json}\n\n{password}")) {
        Ok(id) => Ok(id),
        Err(_) => {
            if !password.is_empty() {
                write_credentials(&place, password)?;
            }
            let mut cfg = Config::load().unwrap_or_default();
            cfg.upsert(place.clone());
            cfg.save()?;
            let _ = mount::enable_place(&place);
            Ok(place.id)
        }
    }
}

pub fn mount_place_sync(id: &str) -> Result<String> {
    call_sync("MountPlace", id)
}

pub fn get_connect_status_sync(id: &str) -> Result<crate::connect::ConnectProgress> {
    match call_sync("GetConnectStatus", id) {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or_else(|_| {
            crate::connect::ConnectProgress::load(id)
        })),
        Err(_) => Ok(crate::connect::ConnectProgress::load(id)),
    }
}

pub fn cancel_connect_sync(id: &str) -> Result<()> {
    call_sync("CancelConnect", id).map(|_| ())
}

pub fn open_mount_logs_sync(id: &str) -> Result<String> {
    call_sync("OpenMountLogs", id)
}

pub fn list_smb_shares_sync(host: &str, user: &str, pass: &str) -> Result<Vec<String>> {
    match call_sync("ListSmbShares", &format!("{host}\n{user}\n{pass}")) {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        Err(_) => mount::smb_list_shares(host, user, pass),
    }
}
