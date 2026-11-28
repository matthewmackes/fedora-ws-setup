use crate::config::{
    cred_path, ensure_mount_root, read_password, Config, CREDS_DIR, MOUNT_ROOT,
};
use crate::place::{Place, Protocol};
use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const UNIT_DIR: &str = "/etc/systemd/system";

/// True only for a real, usable mount. A systemd `autofs` placeholder sitting on the
/// path (automount waiting for first access) is not a mount: reading it can fail
/// with "No such device", so it must not count as connected.
pub fn is_mounted(mount_point: &str) -> bool {
    match fs::read_to_string("/proc/mounts") {
        Ok(mounts) => mounts_contains_real(&mounts, mount_point),
        Err(_) => false,
    }
}

fn mounts_contains_real(proc_mounts: &str, mount_point: &str) -> bool {
    proc_mounts.lines().any(|l| {
        let mut f = l.split_whitespace();
        let target = f.nth(1);
        let fstype = f.next();
        target == Some(mount_point) && fstype != Some("autofs")
    })
}

/// Places mounted by our own helper process (sshfs with a password, the media
/// FUSE daemon) must not get systemd mount/automount units: systemd would run
/// them without the password or helper and fail.
pub fn uses_systemd(place: &Place) -> bool {
    match place.protocol {
        Protocol::Video | Protocol::Audio => false,
        Protocol::Ssh => PathBuf::from(CREDS_DIR).join(format!("{}.key", place.id)).exists(),
        _ => true,
    }
}

pub fn place_reachable(place: &Place) -> bool {
    let host = place.host.trim();
    if host.is_empty() {
        return false;
    }
    let port = place.port.unwrap_or(match place.protocol {
        Protocol::Smb => 445,
        Protocol::Ssh => 22,
        Protocol::Nfs => 2049,
        Protocol::WebDav => {
            if place.url.starts_with("http://") {
                80
            } else {
                443
            }
        }
        Protocol::Video => 8096,
        Protocol::Audio => 4040,
    });
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::new(
            resolve_ip(host).unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
            port,
        ),
        std::time::Duration::from_millis(400),
    )
    .is_ok()
}

pub fn resolve_ip_pub(host: &str) -> Option<std::net::IpAddr> {
    resolve_ip(host)
}

fn resolve_ip(host: &str) -> Option<std::net::IpAddr> {
    use std::net::ToSocketAddrs;
    format!("{host}:0")
        .to_socket_addrs()
        .ok()?
        .next()
        .map(|a| a.ip())
}

/// Generate What=/Where=/Type=/Options= for a Fedora-native mount.
pub fn mount_spec(place: &Place) -> Result<(String, String, String, String)> {
    let where_ = place.mount_point();
    let cred = cred_path(place);
    let extra = place.options.trim();

    match place.protocol {
        Protocol::Smb => {
            let share = if place.path.is_empty() {
                bail!("SMB place needs a share path");
            } else {
                place.path.trim_matches('/')
            };
            let what = format!("//{}/{}", place.host, share);
            // System mounts run as root; world-browse via 0775. Credentials come from the secrets file.
            let mut opts = format!(
                "credentials={},file_mode=0664,dir_mode=0775,iocharset=utf8,_netdev",
                cred.display()
            );
            if !extra.is_empty() {
                opts = format!("{opts},{extra}");
            }
            Ok((what, where_, "cifs".into(), opts))
        }
        Protocol::Nfs => {
            let export = if place.path.is_empty() { "/" } else { &place.path };
            let what = format!("{}:{export}", place.host);
            let mut opts = "rw,_netdev,noatime".to_string();
            if !extra.is_empty() {
                opts = format!("{opts},{extra}");
            }
            Ok((what, where_, "nfs".into(), opts))
        }
        Protocol::Ssh => {
            let remote = if place.path.is_empty() { "." } else { &place.path };
            let user = if place.username.is_empty() {
                place.host.clone()
            } else {
                format!("{}@{}", place.username, place.host)
            };
            let what = format!("{user}:{remote}");
            let port = place.port.unwrap_or(22);
            // Prefer password file via sshfs -o password_stdin handled outside unit when needed;
            // for systemd use ssh keys under CREDS_DIR when named {id}.key
            let key = PathBuf::from(CREDS_DIR).join(format!("{}.key", place.id));
            let mut opts = if key.exists() {
                format!(
                    "allow_other,default_permissions,_netdev,port={port},IdentityFile={},reconnect,\
                     ConnectTimeout=15,ServerAliveInterval=10,ServerAliveCountMax=3,\
                     StrictHostKeyChecking=accept-new",
                    key.display()
                )
            } else {
                format!(
                    "allow_other,default_permissions,_netdev,port={port},reconnect,\
                     ConnectTimeout=15,ServerAliveInterval=10,ServerAliveCountMax=3,\
                     StrictHostKeyChecking=accept-new"
                )
            };
            if !extra.is_empty() {
                opts = format!("{opts},{extra}");
            }
            Ok((what, where_, "fuse.sshfs".into(), opts))
        }
        Protocol::WebDav => {
            let url = if place.url.is_empty() {
                format!(
                    "https://{}/{}",
                    place.host,
                    place.path.trim_start_matches('/')
                )
            } else {
                place.url.clone()
            };
            // davfs2 reads username/password from /etc/davfs2/secrets (see config::write_credentials).
            // Per-place cred file is still written at save time for reference / cleanup.
            let _cred = cred;
            let mut opts = "_netdev,uid=0,gid=0".to_string();
            if !extra.is_empty() {
                opts = format!("{opts},{extra}");
            }
            Ok((url, where_, "davfs".into(), opts))
        }
        Protocol::Video | Protocol::Audio => {
            // Custom FUSE helper; What= is the place id for the daemon fuse launcher.
            let what = format!("neighbourhood-mediafs#{}", place.id);
            let opts = "allow_other,default_permissions,_netdev".to_string();
            Ok((what, where_, "fuse.neighbourhood-mediafs".into(), opts))
        }
    }
}

fn mount_timeouts() -> (u64, u64) {
    Config::load()
        .map(|c| (c.settings.mount_timeout_sec, c.settings.automount_idle_sec))
        .unwrap_or((30, 600))
}

pub fn write_systemd_units(place: &Place) -> Result<()> {
    ensure_mount_root()?;
    if !uses_systemd(place) {
        return Ok(());
    }
    let (what, where_, fstype, mut options) = mount_spec(place)?;
    if let Ok(cfg) = Config::load() {
        if let Some(extra) = cfg.settings.mount_option_presets.get(place.protocol.as_str()) {
            if !extra.is_empty() {
                options = format!("{options},{extra}");
            }
        }
    }
    fs::create_dir_all(&where_)?;

    let stem = systemd_path_unit(&where_);
    let mount_unit = format!("{stem}.mount");
    let automount_unit = format!("{stem}.automount");
    let (timeout, idle) = mount_timeouts();

    let mount_body = format!(
        r#"[Unit]
Description=Network Neighbourhood mount {name}
After=network-online.target
Wants=network-online.target

[Mount]
What={what}
Where={where_}
Type={fstype}
Options={options}
TimeoutSec={timeout}

[Install]
WantedBy=multi-user.target
"#,
        name = place.name,
        what = what,
        where_ = where_,
        fstype = fstype,
        options = options,
        timeout = timeout,
    );

    let automount_body = format!(
        r#"[Unit]
Description=Network Neighbourhood automount {name}

[Automount]
Where={where_}
TimeoutIdleSec={idle}

[Install]
WantedBy=multi-user.target
"#,
        name = place.name,
        where_ = where_,
        idle = idle,
    );

    fs::write(Path::new(UNIT_DIR).join(&mount_unit), mount_body)
        .with_context(|| format!("write {mount_unit}"))?;

    if place.automount {
        fs::write(Path::new(UNIT_DIR).join(&automount_unit), automount_body)
            .with_context(|| format!("write {automount_unit}"))?;
    }

    sync_drive_letter(place)?;
    Ok(())
}

pub fn remove_systemd_units(place: &Place) -> Result<()> {
    let where_ = place.mount_point();
    let stem = systemd_path_unit(&where_);
    let _ = systemctl(&["stop", &format!("{stem}.automount")]);
    let _ = systemctl(&["stop", &format!("{stem}.mount")]);
    let _ = systemctl(&["disable", &format!("{stem}.automount")]);
    let _ = fs::remove_file(Path::new(UNIT_DIR).join(format!("{stem}.automount")));
    let _ = fs::remove_file(Path::new(UNIT_DIR).join(format!("{stem}.mount")));
    if let Some(link) = place.drive_symlink_path() {
        let _ = fs::remove_file(&link);
    }
    Ok(())
}

pub fn sync_drive_letter(place: &Place) -> Result<()> {
    let Some(link) = place.drive_symlink_path() else {
        return Ok(());
    };
    let target = place.mount_point();
    if Path::new(&link).exists() || Path::new(&link).is_symlink() {
        let _ = fs::remove_file(&link);
    }
    symlink(&target, &link).with_context(|| format!("symlink {link} -> {target}"))?;
    Ok(())
}

pub fn daemon_reload() -> Result<()> {
    systemctl(&["daemon-reload"])?;
    Ok(())
}

pub fn enable_place(place: &Place) -> Result<()> {
    if !uses_systemd(place) {
        return Ok(());
    }
    write_systemd_units(place)?;
    daemon_reload()?;
    let stem = systemd_path_unit(&place.mount_point());
    if place.automount {
        systemctl(&["enable", "--now", &format!("{stem}.automount")])?;
    } else {
        systemctl(&["enable", "--now", &format!("{stem}.mount")])?;
    }
    Ok(())
}

pub fn mount_now(place: &Place) -> Result<()> {
    match place.protocol {
        Protocol::Video | Protocol::Audio => mount_media_fuse(place),
        Protocol::Ssh if !PathBuf::from(CREDS_DIR).join(format!("{}.key", place.id)).exists() => {
            mount_sshfs_password(place)
        }
        _ => {
            write_systemd_units(place)?;
            daemon_reload()?;
            let stem = systemd_path_unit(&place.mount_point());
            if place.automount {
                // Trigger automount by touching the path after starting automount unit
                let _ = systemctl(&["start", &format!("{stem}.automount")]);
                let _ = fs::create_dir_all(place.mount_point());
                // Access to trigger
                let _ = fs::read_dir(place.mount_point());
                if !is_mounted(&place.mount_point()) {
                    systemctl(&["start", &format!("{stem}.mount")])?;
                }
            } else {
                systemctl(&["start", &format!("{stem}.mount")])?;
            }
            sync_drive_letter(place)?;
            Ok(())
        }
    }
}

pub fn unmount_now(place: &Place) -> Result<()> {
    let mp = place.mount_point();
    let stem = systemd_path_unit(&mp);
    let _ = systemctl(&["stop", &format!("{stem}.mount")]);
    // Fallback
    if is_mounted(&mp) {
        let _ = Command::new("umount").arg(&mp).status();
        let _ = Command::new("fusermount3").args(["-u", &mp]).status();
        let _ = Command::new("fusermount").args(["-u", &mp]).status();
    }
    Ok(())
}

fn mount_sshfs_password(place: &Place) -> Result<()> {
    ensure_mount_root()?;
    let mp = place.mount_point();
    fs::create_dir_all(&mp)?;
    if is_mounted(&mp) {
        return Ok(());
    }
    // sshfs treats bare "/" fine; empty path means home/default.
    let remote_path = if place.path.is_empty() {
        "."
    } else {
        place.path.as_str()
    };
    let userhost = if place.username.is_empty() {
        place.host.clone()
    } else {
        format!("{}@{}", place.username, place.host)
    };
    let remote = format!("{userhost}:{remote_path}");
    let password = read_password(place).unwrap_or_default();
    if password.is_empty() {
        bail!("SSH password is empty — use Edit credentials and Save & Retry");
    }
    let port = place.port.unwrap_or(22);
    if !place_reachable(place) {
        bail!(
            "Host {host} port {port} is not reachable — check VPN/network, then Retry",
            host = place.host
        );
    }

    // Force password auth so sshpass sees a prompt immediately (avoids pubkey stall).
    let opts = format!(
        "allow_other,default_permissions,reconnect,\
         StrictHostKeyChecking=accept-new,ConnectTimeout=15,\
         ServerAliveInterval=10,ServerAliveCountMax=3,port={port},\
         PubkeyAuthentication=no,PreferredAuthentications=password,\
         NumberOfPasswordPrompts=1"
    );

    // password_stdin is unreliable with sshfs 3.x here; sshpass works.
    // Run sshfs in foreground (-f) under our process tree, then return once mounted
    // (do not wait for sshfs exit — it must keep running).
    let sshpass = Path::new("/usr/bin/sshpass");
    let mut cmd = if sshpass.exists() {
        // Prefer env (-e) so the password never appears on argv /proc.
        let mut c = Command::new(sshpass);
        c.env("SSHPASS", &password);
        c.args(["-e", "sshfs", "-f", &remote, &mp, "-o", &opts]);
        c
    } else {
        // Fallback: password_stdin (may fail on some sshfs builds).
        let mut c = Command::new("sshfs");
        c.args([
            "-f",
            &remote,
            &mp,
            "-o",
            &format!("{opts},password_stdin"),
        ]);
        c.stdin(Stdio::piped());
        c
    };
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    tracing::info!(
        place = %place.id,
        "exec: {}sshfs -f {remote} {mp} -o {opts}  (password via {})",
        if sshpass.exists() { "SSHPASS=*** sshpass -e " } else { "" },
        if sshpass.exists() { "env" } else { "stdin" },
    );
    let mut child = cmd.spawn().context("spawn sshfs")?;
    if !sshpass.exists() {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = writeln!(stdin, "{password}");
            drop(stdin);
        }
    }

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(45);
    loop {
        if crate::connect::is_cancelled(&place.id) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = unmount_now(place);
            bail!("connect cancelled");
        }
        if is_mounted(&mp) {
            // sshfs registers the FUSE mount before the login is confirmed, so a
            // wrong password still shows up here briefly. Require the helper to
            // stay alive and the folder to be readable before calling it connected.
            if let Some(err) = settle_check(&mut child, &mp) {
                let _ = unmount_now(place);
                bail!("{err}");
            }
            // Keep sshfs running; a watcher thread reaps it and journals why it exited.
            watch_helper(child, place.id.clone(), mp.clone());
            sync_drive_letter(place)?;
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            let err = child
                .stderr
                .take()
                .and_then(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok()?;
                    Some(buf)
                })
                .unwrap_or_default();
            let hint = if err.to_ascii_lowercase().contains("permission denied") {
                " — use Edit credentials to update the password"
            } else {
                ""
            };
            bail!(
                "sshfs exited ({status}): {}{hint}",
                err.trim().chars().take(400).collect::<String>()
            );
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("sshfs timed out after {}s — host hung or auth stalled; Retry or Edit credentials", timeout.as_secs());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// After the mount first appears, watch it for a moment. Returns the failure
/// text if the helper exits (e.g. "Permission denied") or the folder is unreadable.
fn settle_check(child: &mut std::process::Child, mp: &str) -> Option<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2500);
    while std::time::Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            let err = child
                .stderr
                .take()
                .and_then(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok()?;
                    Some(buf)
                })
                .unwrap_or_default();
            return Some(format!(
                "sshfs exited ({status}): {}",
                err.trim().chars().take(400).collect::<String>()
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    match std::fs::read_dir(mp) {
        Ok(_) => None,
        Err(e) => Some(format!("mounted folder is not readable: {e}")),
    }
}

/// Wait for a long-running mount helper in the background so it is reaped and
/// its exit status and stderr reach the journal.
fn watch_helper(mut child: std::process::Child, id: String, mp: String) {
    std::thread::spawn(move || {
        let stderr = child.stderr.take();
        let status = child.wait();
        let mut err = String::new();
        if let Some(mut s) = stderr {
            let _ = std::io::Read::read_to_string(&mut s, &mut err);
        }
        tracing::warn!(
            place = %id,
            mount = %mp,
            status = ?status,
            stderr = %crate::connect::redact(err.trim()),
            "mount helper exited"
        );
    });
}

fn mount_media_fuse(place: &Place) -> Result<()> {
    ensure_mount_root()?;
    let mp = place.mount_point();
    fs::create_dir_all(&mp)?;
    if is_mounted(&mp) {
        return Ok(());
    }
    // Spawn daemon helper: network-neighbourhoodd --mediafs <id> <mountpoint>
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("network-neighbourhoodd"));
    tracing::info!(place = %place.id, "exec: {} --mediafs {} {mp}", exe.display(), place.id);
    let mut child = Command::new(exe)
        .args(["--mediafs", &place.id, &mp])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn mediafs")?;

    // Wait for the FUSE mount to appear, or for the helper to explain why it exited.
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(60);
    loop {
        if crate::connect::is_cancelled(&place.id) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("connect cancelled");
        }
        if is_mounted(&mp) {
            watch_helper(child, place.id.clone(), mp.clone());
            sync_drive_letter(place)?;
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            let err = child
                .stderr
                .take()
                .and_then(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok()?;
                    Some(buf)
                })
                .unwrap_or_default();
            bail!(
                "media mount helper exited ({status}): {}",
                err.trim().chars().take(400).collect::<String>()
            );
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "media server timed out after {}s while listing the library",
                timeout.as_secs()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// `systemd-escape -p` equivalent: `/` becomes `-`; only `[A-Za-z0-9:_.]` stay
/// literal (a leading `.` is escaped); everything else, including `-`, is `\xNN`.
pub fn systemd_path_unit(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return "-".into();
    }
    let mut out = String::new();
    for (i, c) in trimmed.chars().enumerate() {
        match c {
            '/' => out.push('-'),
            '.' if i == 0 => out.push_str("\\x2e"),
            c if c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '.') => out.push(c),
            c => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("\\x{b:02x}"));
                }
            }
        }
    }
    out
}

fn systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl").args(args).status()?;
    if !status.success() {
        bail!("systemctl {} failed: {status}", args.join(" "));
    }
    Ok(())
}

pub fn protocol_dirs_exist() -> bool {
    Path::new(MOUNT_ROOT).join("SMB").is_dir()
}

/// Mount with verbose protocol steps written to the connect progress store.
pub fn mount_now_with_log(place: &Place) -> (Result<()>, Vec<String>) {
    use crate::connect::{Cancelled, ProgressReporter};

    let mp = place.mount_point();
    let mut reporter = ProgressReporter::start(&place.id, &mp);
    let mut log = Vec::new();

    let push = |log: &mut Vec<String>, line: String| {
        log.push(line);
    };

    if is_mounted(&mp) {
        reporter.already_mounted();
        push(
            &mut log,
            format!("Already mounted — {mp}"),
        );
        return (Ok(()), log);
    }

    let auth_method = match place.protocol {
        Protocol::Ssh => {
            let key = PathBuf::from(CREDS_DIR).join(format!("{}.key", place.id));
            if key.exists() {
                "SSH key"
            } else {
                "password"
            }
        }
        Protocol::Smb | Protocol::WebDav => {
            if place.username.is_empty() {
                "guest / anonymous"
            } else {
                "password"
            }
        }
        Protocol::Nfs => "host-based (NFS)",
        Protocol::Video | Protocol::Audio => "media token / URL",
    };
    let user_disp = if place.username.is_empty() {
        "(none)"
    } else {
        place.username.as_str()
    };
    reporter.set_auth(user_disp, auth_method);

    let run = (|| -> Result<()> {
        push(
            &mut log,
            format!(
                "Mounting {} ({}) → {}",
                place.name,
                place.protocol.group_label(),
                mp
            ),
        );

        reporter.step(
            "Resolving host",
            &format!("DNS/mDNS lookup for {}", place.host),
            0.12,
        )?;
        let ip = resolve_ip(&place.host);
        let resolve_detail = match ip {
            Some(ip) => format!("{} → {ip}", place.host),
            None => format!("{} (no A/AAAA yet — continuing)", place.host),
        };
        push(&mut log, format!("Resolve: {resolve_detail}"));
        reporter.step("Host resolved", &resolve_detail, 0.20)?;

        let port = place.port.unwrap_or(match place.protocol {
            Protocol::Smb => 445,
            Protocol::Ssh => 22,
            Protocol::Nfs => 2049,
            Protocol::WebDav => {
                if place.url.starts_with("http://") {
                    80
                } else {
                    443
                }
            }
            Protocol::Video => 8096,
            Protocol::Audio => 4040,
        });
        reporter.step(
            "Checking TCP reachability",
            &format!("Probing {}:{}", place.host, port),
            0.32,
        )?;
        let reachable = place_reachable(place);
        push(
            &mut log,
            format!(
                "TCP {}:{} — {}",
                place.host,
                port,
                if reachable { "open" } else { "unreachable" }
            ),
        );
        if !reachable && matches!(place.protocol, Protocol::Ssh | Protocol::Smb | Protocol::Nfs) {
            bail!("Host {}:{} is not reachable — check VPN/network, then Retry", place.host, port);
        }
        reporter.step(
            "TCP check",
            &format!(
                "{}:{} {}",
                place.host,
                port,
                if reachable { "reachable" } else { "skipped / soft-fail" }
            ),
            0.40,
        )?;

        reporter.step(
            "Authenticating",
            &format!("method={auth_method}; user={user_disp}"),
            0.52,
        )?;
        push(
            &mut log,
            format!("Auth: {auth_method} as {user_disp}"),
        );

        let helper = match place.protocol {
            Protocol::Smb => "mount.cifs / systemd .mount",
            Protocol::Ssh => "sshfs (+ sshpass if password)",
            Protocol::Nfs => "mount.nfs / systemd .mount",
            Protocol::WebDav => "davfs2 / systemd .mount",
            Protocol::Video | Protocol::Audio => "neighbourhood-mediafs",
        };
        reporter.step(
            "Starting mount helper",
            &format!("{helper}; path={}", place.path),
            0.68,
        )?;
        push(&mut log, format!("Helper: {helper}"));

        if crate::connect::is_cancelled(&place.id) {
            return Err(Cancelled.into());
        }

        mount_now(place)?;

        if crate::connect::is_cancelled(&place.id) {
            let _ = unmount_now(place);
            return Err(Cancelled.into());
        }

        reporter.step(
            "Verifying mount path",
            &format!("Checking {mp}"),
            0.88,
        )?;
        let mounted = is_mounted(&mp);
        push(
            &mut log,
            if mounted {
                format!("Path check: mounted at {mp}")
            } else {
                format!("Path check: unit started; {mp} may automount on access")
            },
        );
        let path_detail = if mounted {
            format!("Mounted at {mp}")
        } else {
            format!("Ready (automount) at {mp}")
        };
        reporter.step("Path check", &path_detail, 0.96)?;
        Ok(())
    })();

    match &run {
        Ok(()) => {
            let mounted = is_mounted(&mp);
            let phrase = if mounted {
                format!("Connected — {mp}")
            } else {
                format!("Connected (automount) — {mp}")
            };
            reporter.succeed(&phrase);
            push(&mut log, phrase);
            if let Ok(out) = Command::new("systemctl")
                .args([
                    "status",
                    "--no-pager",
                    "-l",
                    &format!("{}.mount", systemd_path_unit(&mp)),
                ])
                .output()
            {
                for line in String::from_utf8_lossy(&out.stdout).lines().take(12) {
                    push(&mut log, line.to_string());
                }
            }
        }
        Err(e) => {
            if crate::connect::is_cancelled(&place.id) || e.downcast_ref::<Cancelled>().is_some() {
                let _ = unmount_now(place);
                reporter.mark_cancelled();
                push(&mut log, "Cancelled — cleaned up partial mount".into());
            } else {
                let msg = e.to_string();
                reporter.fail_step("Mount failed", &msg);
                push(&mut log, format!("ERROR: {msg}"));
                if let Ok(out) = Command::new("journalctl")
                    .args([
                        "-u",
                        &format!("{}.mount", systemd_path_unit(&mp)),
                        "-n",
                        "30",
                        "--no-pager",
                    ])
                    .output()
                {
                    for line in String::from_utf8_lossy(&out.stdout).lines() {
                        push(&mut log, line.to_string());
                    }
                }
            }
        }
    }
    if !matches!(&run, Err(e) if crate::connect::is_cancelled(&place.id)
        || e.downcast_ref::<Cancelled>().is_some())
    {
        let progress = crate::connect::ConnectProgress::load(&place.id);
        crate::status::record(&crate::status::HistoryEntry {
            id: place.id.clone(),
            name: place.name.clone(),
            ts: crate::status::now_secs(),
            ok: run.is_ok(),
            phrase: progress.phrase.clone(),
            error: progress.error.clone(),
            cause: progress.diagnosis.cause.clone(),
            steps: progress
                .steps
                .iter()
                .map(|s| format!("{} {} — {}", if s.ok { "✓" } else { "✗" }, s.label, s.detail))
                .collect(),
        });
    }
    (run, log)
}

/// Cancel an in-flight connect and tear down any partial mount.
pub fn cancel_connect(place: &Place) -> Result<()> {
    crate::connect::request_cancel(&place.id);
    let _ = unmount_now(place);
    let mut progress = crate::connect::ConnectProgress::load(&place.id);
    progress.state = crate::connect::ConnectState::Cancelled;
    progress.phrase = "Cancelled".into();
    progress.error = "Connect cancelled — cleaned up".into();
    progress.save();
    crate::connect::end_connect(&place.id);
    Ok(())
}

/// Disable automount unit but keep the place configured.
pub fn disable_automount(place: &Place) -> Result<()> {
    let stem = systemd_path_unit(&place.mount_point());
    let _ = systemctl(&["stop", &format!("{stem}.automount")]);
    let _ = systemctl(&["disable", &format!("{stem}.automount")]);
    let _ = unmount_now(place);
    Ok(())
}

/// List SMB shares on a host via smbclient.
pub fn smb_list_shares(host: &str, username: &str, password: &str) -> Result<Vec<String>> {
    let mut cmd = Command::new("smbclient");
    cmd.args(["-L", host, "-g"]);
    if username.is_empty() {
        cmd.arg("-N");
    } else {
        cmd.arg("-U").arg(format!("{username}%{password}"));
    }
    cmd.stdin(Stdio::null());
    let output = cmd.output().context("smbclient -L")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() && stdout.is_empty() {
        bail!("smbclient failed: {stderr}");
    }
    let mut shares = Vec::new();
    for line in stdout.lines().chain(stderr.lines()) {
        // Disk|share|comment  or  \tSharename
        if let Some(rest) = line.strip_prefix("Disk|") {
            if let Some(name) = rest.split('|').next() {
                let name = name.trim();
                if !name.is_empty() && !name.ends_with('$') {
                    shares.push(name.to_string());
                }
            }
        } else if line.to_ascii_lowercase().contains("disk") {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0].eq_ignore_ascii_case("Disk") {
                // skip header
                continue;
            }
            if parts.len() >= 1 {
                let name = parts[0].trim();
                if !name.is_empty()
                    && !name.eq_ignore_ascii_case("Sharename")
                    && !name.eq_ignore_ascii_case("Disk")
                    && !name.ends_with('$')
                {
                    shares.push(name.to_string());
                }
            }
        }
    }
    shares.sort();
    shares.dedup();
    Ok(shares)
}

pub fn open_in_files(path: &str) -> Result<()> {
    // Never .status() — waiting on the file manager freezes callers.
    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("xdg-open")?;
    Ok(())
}

/// Ensure a short-lived privileged session via polkit (auth_admin_keep).
pub fn ensure_admin_session() -> Result<()> {
    let status = Command::new("pkcheck")
        .args([
            "--action-id",
            "org.mde.NetworkNeighbourhood1.manage",
            "--process",
            &format!("{}:{}", std::process::id(), 0),
            "--allow-user-interaction",
        ])
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) | Err(_) => {
            // Fallback: pkexec true establishes keep credentials for root helpers
            let mut child = Command::new("pkexec")
                .arg("true")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .context("pkexec")?;
            let st = child.wait()?;
            if st.success() {
                Ok(())
            } else {
                bail!("administrator authentication failed")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autofs_placeholder_is_not_a_mount() {
        let m = "systemd-1 /mnt/x autofs rw 0 0\nsrv:/a /mnt/y nfs rw 0 0\nfoo /mnt/x fuse.sshfs rw 0 0\n";
        assert!(mounts_contains_real(m, "/mnt/x"), "real mount stacked on autofs counts");
        assert!(mounts_contains_real(m, "/mnt/y"));
        assert!(!mounts_contains_real("systemd-1 /mnt/x autofs rw 0 0\n", "/mnt/x"));
        assert!(!mounts_contains_real(m, "/mnt/none"));
    }

    #[test]
    fn helper_mounted_places_skip_systemd() {
        assert!(!uses_systemd(&Place::new("m", Protocol::Audio, "h")));
        assert!(!uses_systemd(&Place::new("m", Protocol::Video, "h")));
        assert!(!uses_systemd(&Place::new("s", Protocol::Ssh, "h")), "no key yet");
        assert!(uses_systemd(&Place::new("n", Protocol::Nfs, "h")));
    }

    #[test]
    fn escape_mount_path() {
        let u = systemd_path_unit("/mnt/network/SMB/nas/media");
        assert_eq!(u, "mnt-network-SMB-nas-media");
        // Must match `systemd-escape -p` exactly or systemd refuses the unit
        // ("Where= setting doesn't match unit name").
        assert_eq!(
            systemd_path_unit("/mnt/network/SSH/172.20.0.2/ssh_172.20.0.2"),
            "mnt-network-SSH-172.20.0.2-ssh_172.20.0.2"
        );
        assert_eq!(
            systemd_path_unit("/mnt/network/SMB/NAS.local/My Share"),
            "mnt-network-SMB-NAS.local-My\\x20Share"
        );
        assert_eq!(systemd_path_unit("/mnt/network/NFS/a-b/x"), "mnt-network-NFS-a\\x2db-x");
    }
}
