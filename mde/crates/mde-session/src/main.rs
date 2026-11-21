//! mde-session — user session after greeter handoff.
//! Starts mde-comp (DRM), waits for WAYLAND_DISPLAY, then wallpaper + mde-shell.
//!
//! Subcommands (also available as `mde-session-ctl <cmd>`):
//!   unlock | wallpaper | shell | chrome | recover | comp | status | help
use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

mod chrome;

fn main() -> Result<()> {
    mde_config::supervisor::install_panic_hook();
    prepend_local_bin();

    let mut args = env::args().skip(1);
    if let Some(cmd) = args.next() {
        return ctl_main(&cmd, args.collect());
    }
    session_main()
}

fn ctl_main(cmd: &str, _rest: Vec<String>) -> Result<()> {
    match cmd {
        "help" | "-h" | "--help" => {
            print_ctl_help();
            Ok(())
        }
        "status" => {
            print_status();
            Ok(())
        }
        "unlock" => {
            // Prefer in-session handling when a modern supervisor is running; also
            // act locally so recovery works against older /usr/bin/mde-session.
            let _ = chrome::write_session_cmd("unlock");
            chrome::unlock_seat()
        }
        "wallpaper" => {
            let _ = chrome::write_session_cmd("wallpaper");
            let wall = chrome::configured_wallpaper();
            chrome::restart_wallpaper(&wall)
        }
        "shell" => {
            let _ = chrome::write_session_cmd("shell");
            chrome::restart_shell()
        }
        "chrome" => {
            let _ = chrome::write_session_cmd("chrome");
            chrome::restart_chrome()
        }
        "comp" => {
            let _ = chrome::write_session_cmd("comp");
            chrome::unlock_seat()?;
            chrome::restart_comp()
        }
        "recover" => {
            // Black-screen recovery: unlock + bounce compositor so the session
            // respawns wallpaper and shell on a fresh seat.
            let _ = chrome::write_session_cmd("recover");
            chrome::recover_seat()
        }
        other => {
            eprintln!("unknown command: {other}");
            print_ctl_help();
            bail!("unknown command: {other}");
        }
    }
}

fn print_ctl_help() {
    eprintln!(
        "\
mde-session / mde-session-ctl — MDE chrome recovery

Usage:
  mde-session                 Run the user session (greetd/seatd)
  mde-session <command>       Same as mde-session-ctl
  mde-session-ctl <command>

Commands:
  status      Show compositor / shell / lock / wallpaper state
  unlock      Unlock seat and stop mde-lock (safe)
  wallpaper   Restart swaybg from theme.wallpaper
  shell       Restart mde-shell (session respawns it)
  chrome      unlock + wallpaper + shell
  comp        unlock + restart mde-comp (session respawns chrome)
  recover     Full black-screen recovery (unlock + comp bounce)
  help        Show this help
"
    );
}

fn print_status() {
    let display = chrome::read_wayland_display()
        .map(|d| format!("ok ({d})"))
        .unwrap_or_else(|e| format!("missing ({e})"));
    let comp = pgrep_one("mde-comp");
    let shell = pgrep_one("mde-shell");
    let lock = pgrep_one("mde-lock");
    let wall = if chrome::wallpaper_alive() {
        "alive"
    } else {
        "dead"
    };
    let locked = chrome::lock_flag_path().is_file();
    println!("wayland:   {display}");
    println!("mde-comp:  {comp}");
    println!("mde-shell: {shell}");
    println!("mde-lock:  {lock}");
    println!("wallpaper: {wall}");
    println!("lock flag: {}", if locked { "set" } else { "clear" });
}

fn pgrep_one(name: &str) -> String {
    let out = Command::new("pgrep").args(["-a", "-x", name]).output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "not running".into(),
    }
}

fn session_main() -> Result<()> {
    env::set_var("XDG_CURRENT_DESKTOP", "MDE");
    env::set_var("XDG_SESSION_DESKTOP", "mde");
    env::set_var("XDG_SESSION_TYPE", "wayland");
    env::set_var("XDG_DESKTOP_PORTAL_DIR", "/usr/share/xdg-desktop-portal");
    env::set_var("MDE_VERSION", mde_theme::VERSION);
    env::set_var("MOZ_ENABLE_WAYLAND", "1");
    env::set_var("ELECTRON_OZONE_PLATFORM_HINT", "auto");
    // Prefer compositor SSD; discourage GTK client-side decorations / HeaderBars.
    env::set_var("GTK_CSD", "0");
    if env::var_os("QT_QPA_PLATFORM").is_none() {
        env::set_var("QT_QPA_PLATFORM", "wayland;xcb");
    }
    if env::var_os("SDL_VIDEODRIVER").is_none() {
        env::set_var("SDL_VIDEODRIVER", "wayland,x11");
    }
    env::set_var("_JAVA_AWT_WM_NONREPARENTING", "1");
    // Start → Safe mode writes ~/.local/state/mde/safe.req for the next launch (W54).
    if env::var_os("MDE_SAFE").is_none() {
        if let Ok(home) = env::var("HOME") {
            let req = PathBuf::from(home).join(".local/state/mde/safe.req");
            if req.is_file() {
                let _ = fs::remove_file(&req);
                env::set_var("MDE_SAFE", "1");
                chrome::session_log("safe mode: honored safe.req from Start");
            }
        }
    }
    if env::var_os("MDE_SAFE").is_some() {
        chrome::session_log("safe mode: night light skipped, shell idle lock extended");
    }

    let cfg = mde_config::MdeConfig::load().unwrap_or_default();
    if cfg.active().session.xwayland {
        env::set_var("WLR_XWAYLAND", "1");
    } else {
        env::remove_var("WLR_XWAYLAND");
    }

    ensure_config()?;
    if env::var_os("MDE_SAFE").is_none() && cfg.active().session.night_light {
        let _ = start_night_light();
    }

    let session_started = Instant::now();
    chrome::session_log("mde-budget session_spawn_ms=0");

    let marker = chrome::wayland_marker();
    let _ = fs::remove_file(&marker);

    let mut comp = spawn_args("mde-comp", &["--session"])?;
    chrome::session_log(&format!(
        "mde-budget comp_spawned_ms={}",
        session_started.elapsed().as_millis()
    ));
    let mut display = wait_for_wayland(&marker, &mut comp, Duration::from_secs(10))?;
    chrome::session_log(&format!(
        "mde-budget wayland_ready_ms={}",
        session_started.elapsed().as_millis()
    ));
    env::set_var("WAYLAND_DISPLAY", &display);
    if env::var_os("MDE_SAFE").is_none() {
        let wall = cfg.active().theme.wallpaper.clone();
        if !wall.trim().is_empty() {
            if let Err(e) = chrome::restart_wallpaper(&wall) {
                chrome::session_log(&format!("wallpaper: {e}"));
            }
        }
    }

    let mut shell = spawn_shell(&display)?;
    chrome::session_log(&format!(
        "mde-budget shell_spawned_ms={}",
        session_started.elapsed().as_millis()
    ));
    // Allow a burst of chrome crashes, then cool down (W64).
    const SHELL_RESPAWN_MAX: u32 = 8;
    const SHELL_STABLE_SECS: u64 = 60;
    let mut shell_restarts: u32 = 0;
    let mut shell_since = Instant::now();
    let mut comp_restarts: u32 = 0;
    let mut wallpaper_check = Instant::now();

    loop {
        thread::sleep(Duration::from_millis(400));

        // External recovery requests from mde-session-ctl / `mde-session <cmd>`.
        if let Some(cmd) = chrome::take_session_cmd() {
            chrome::session_log(&format!("session cmd: {cmd}"));
            match cmd.as_str() {
                "unlock" => {
                    let _ = chrome::unlock_seat();
                }
                "wallpaper" => {
                    let wall = cfg.active().theme.wallpaper.clone();
                    if let Err(e) = chrome::restart_wallpaper(&wall) {
                        chrome::session_log(&format!("wallpaper cmd: {e}"));
                    }
                }
                "shell" => {
                    let _ = shell.kill();
                    let _ = shell.wait();
                }
                "chrome" => {
                    let _ = chrome::unlock_seat();
                    let wall = cfg.active().theme.wallpaper.clone();
                    if let Err(e) = chrome::restart_wallpaper(&wall) {
                        chrome::session_log(&format!("chrome wallpaper: {e}"));
                    }
                    let _ = shell.kill();
                    let _ = shell.wait();
                }
                "comp" | "recover" => {
                    let _ = chrome::unlock_seat();
                    let _ = comp.kill();
                    // Fall through to the normal comp-exit restart path below.
                }
                other => chrome::session_log(&format!("unknown session cmd: {other}")),
            }
        }

        // Wallpaper watchdog — dead swaybg leaves a black CLEAR_COLOR seat.
        if env::var_os("MDE_SAFE").is_none()
            && wallpaper_check.elapsed() > Duration::from_secs(2)
        {
            wallpaper_check = Instant::now();
            if !chrome::wallpaper_alive() {
                let wall = cfg.active().theme.wallpaper.clone();
                if !wall.trim().is_empty() {
                    chrome::session_log("wallpaper watchdog: swaybg dead — restarting");
                    if let Err(e) = chrome::restart_wallpaper(&wall) {
                        chrome::session_log(&format!("wallpaper watchdog: {e}"));
                    }
                }
            }
            // Orphan mde-lock (no lock flag) holds ext-session-lock with a black UI.
            let lock_alive = Command::new("pgrep")
                .args(["-x", "mde-lock"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if lock_alive && !chrome::lock_flag_path().is_file() {
                chrome::session_log("lock watchdog: orphan mde-lock without flag — unlocking");
                let _ = chrome::unlock_seat();
            }
        }

        match comp.try_wait() {
            Ok(Some(status)) => {
                let _ = shell.kill();
                match mde_config::supervisor::comp_after_exit(comp_restarts) {
                    mde_config::supervisor::CompAction::Stop => {
                        chrome::session_log(&format!("mde-comp exited: {status}; giving up"));
                        bail!("mde-comp exited: {status}");
                    }
                    mde_config::supervisor::CompAction::Restart { attempt } => {
                        chrome::session_log(&mde_config::supervisor::comp_restart_line(
                            attempt,
                            &status.to_string(),
                        ));
                        thread::sleep(Duration::from_secs(
                            mde_config::supervisor::COMP_RESTART_GAP_SECS,
                        ));
                        let _ = fs::remove_file(&marker);
                        comp = spawn_args("mde-comp", &["--session"])?;
                        display = wait_for_wayland(&marker, &mut comp, Duration::from_secs(10))?;
                        env::set_var("WAYLAND_DISPLAY", &display);
                        // swaybg dies with the old compositor; restart it or the seat stays
                        // on CLEAR_COLOR until a window opens.
                        if env::var_os("MDE_SAFE").is_none() {
                            let wall = cfg.active().theme.wallpaper.clone();
                            if !wall.trim().is_empty() {
                                if let Err(e) = chrome::restart_wallpaper(&wall) {
                                    chrome::session_log(&format!(
                                        "wallpaper (after comp restart): {e}"
                                    ));
                                }
                            }
                        }
                        shell = spawn_shell(&display)?;
                        comp_restarts = attempt;
                        shell_restarts = 0;
                        shell_since = Instant::now();
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                let _ = shell.kill();
                bail!("mde-comp wait error: {e}");
            }
        }
        match shell.try_wait() {
            Ok(Some(status)) => {
                chrome::session_log(&format!("mde-shell exited: {status}"));
                if shell_restarts >= SHELL_RESPAWN_MAX {
                    // Storm exhausted — wait for cooldown, reset counter, try again (W64).
                    chrome::session_log(&format!(
                        "mde-shell crash storm ({SHELL_RESPAWN_MAX}×); cooling {SHELL_STABLE_SECS}s then retry"
                    ));
                    thread::sleep(Duration::from_secs(SHELL_STABLE_SECS));
                    shell_restarts = 0;
                }
                shell_restarts += 1;
                thread::sleep(Duration::from_millis(300));
                match spawn_shell(&display) {
                    Ok(child) => {
                        shell = child;
                        shell_since = Instant::now();
                        chrome::session_log(&mde_config::supervisor::shell_respawn_line(
                            shell_restarts,
                        ));
                    }
                    Err(e) => {
                        let _ = comp.kill();
                        return Err(e);
                    }
                }
            }
            Ok(None) => {
                // Idle success: after 60s of stable chrome, clear the storm counter.
                if shell_restarts > 0 && shell_since.elapsed() > Duration::from_secs(SHELL_STABLE_SECS)
                {
                    chrome::session_log("mde-shell stable 60s — respawn counter reset");
                    shell_restarts = 0;
                }
            }
            Err(_) => {
                let _ = comp.kill();
                break;
            }
        }
    }

    let _ = comp.kill();
    let _ = shell.kill();
    let _ = fs::remove_file(&marker);
    Ok(())
}

fn start_night_light() -> Result<()> {
    let runtime = chrome::runtime_dir();
    let pid_path = runtime.join("mde-night-light.pid");
    let child = Command::new("wlsunset")
        .args(["-t", "4000", "-T", "6500"])
        .spawn()
        .or_else(|_| Command::new("gammastep").args(["-O", "4500"]).spawn())?;
    let _ = fs::write(pid_path, child.id().to_string());
    std::mem::forget(child);
    Ok(())
}

fn prepend_local_bin() {
    let Ok(home) = env::var("HOME") else {
        return;
    };
    let local = PathBuf::from(home).join(".local/bin");
    if !local.is_dir() {
        return;
    }
    let local_s = local.to_string_lossy().into_owned();
    let path = env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    if path.split(':').any(|p| p == local_s) {
        return;
    }
    env::set_var("PATH", format!("{local_s}:{path}"));
}

fn spawn_shell(display: &str) -> Result<Child> {
    Command::new("mde-shell")
        .env("WAYLAND_DISPLAY", display)
        .env("XDG_RUNTIME_DIR", chrome::runtime_dir())
        .spawn()
        .context("spawn mde-shell (is it on PATH?)")
}

fn wait_for_wayland(marker: &PathBuf, comp: &mut Child, timeout: Duration) -> Result<String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(name) = fs::read_to_string(marker) {
            let name = name.trim().to_string();
            if !name.is_empty() {
                return Ok(name);
            }
        }
        match comp.try_wait() {
            Ok(Some(status)) => bail!("mde-comp exited before Wayland ready: {status}"),
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => bail!("mde-comp wait error: {e}"),
        }
    }
    let _ = comp.kill();
    bail!("timed out waiting for {marker:?}")
}

fn spawn_args(bin: &str, args: &[&str]) -> Result<Child> {
    Command::new(bin)
        .args(args)
        .spawn()
        .with_context(|| format!("spawn {bin} (is it on PATH?)"))
}

fn ensure_config() -> Result<()> {
    let mut cfg = mde_config::MdeConfig::load().unwrap_or_default();
    let _ = cfg.migrate_legacy();
    cfg.save().context("write mde config")?;
    let dir = mde_config::MdeConfig::config_dir()?;
    fs::create_dir_all(dir.join("profiles"))?;
    Ok(())
}
