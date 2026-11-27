//! Workstation setup UI — tri-state catalog + live apply progress.
use anyhow::{bail, Context, Result};
use clap::Parser;
use iced::border::Radius;
use iced::widget::{
    button, checkbox, column, container, progress_bar, radio, row, scrollable, text, Space,
};
use iced::{
    Background, Border, Color, Element, Font, Length, Padding, Shadow, Task, Theme, Vector,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tracing::{info, warn};

// --- palette (Win11-adjacent charcoal, not purple) -----------------------

const BG: Color = Color::from_rgb(0.10, 0.11, 0.13);
const SURFACE: Color = Color::from_rgb(0.16, 0.17, 0.20);
const SURFACE2: Color = Color::from_rgb(0.20, 0.21, 0.25);
const BORDER: Color = Color::from_rgb(0.28, 0.30, 0.34);
const TEXT: Color = Color::from_rgb(0.95, 0.95, 0.96);
const MUTED: Color = Color::from_rgb(0.62, 0.64, 0.68);
const ACCENT: Color = Color::from_rgb(0.20, 0.55, 0.90);
const ACCENT_HOVER: Color = Color::from_rgb(0.28, 0.62, 0.95);
const OK: Color = Color::from_rgb(0.30, 0.72, 0.45);
const WARN: Color = Color::from_rgb(0.92, 0.68, 0.22);
const DANGER: Color = Color::from_rgb(0.90, 0.35, 0.32);
const INSTALL_C: Color = Color::from_rgb(0.35, 0.70, 0.95);
const KEEP_C: Color = Color::from_rgb(0.45, 0.78, 0.55);
const REMOVE_C: Color = Color::from_rgb(0.95, 0.50, 0.40);

#[derive(Debug, Parser)]
#[command(name = "mde-setup-ui")]
struct Args {
    #[arg(long)]
    model: PathBuf,
    #[arg(long)]
    script: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct Model {
    dry_run: bool,
    log: String,
    #[serde(default)]
    #[allow(dead_code)]
    fedora: String,
    default_de: String,
    #[serde(default)]
    mde_profile: String,
    users: Vec<String>,
    #[serde(default)]
    force_surface: String,
    #[serde(default)]
    no_surface: bool,
    #[serde(default)]
    simulate_surface: String,
    #[serde(default)]
    #[allow(dead_code)]
    desktops: BTreeMap<String, DesktopInfo>,
    categories: BTreeMap<String, Vec<Category>>,
}

#[derive(Debug, Clone, Deserialize)]
struct DesktopInfo {
    #[serde(default)]
    #[allow(dead_code)]
    base: String,
    #[serde(default)]
    #[allow(dead_code)]
    switch: bool,
    #[serde(default)]
    #[allow(dead_code)]
    switch_text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Category {
    title: String,
    #[allow(dead_code)]
    key: String,
    items: Vec<CatItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatItem {
    id: String,
    label: String,
    on: bool,
    #[serde(default)]
    present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemIntent {
    Leave,
    Install,
    Keep,
    Remove,
}

impl ItemIntent {
    fn badge(self) -> (&'static str, Color) {
        match self {
            Self::Leave => ("leave", MUTED),
            Self::Install => ("install", INSTALL_C),
            Self::Keep => ("keep", KEEP_C),
            Self::Remove => ("remove", REMOVE_C),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Hub,
    Desktop,
    Users,
    Category(usize),
    Progress,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeChoice {
    Xfce,
    Mde,
    Pipedal,
}
impl DeChoice {
    fn as_str(self) -> &'static str {
        match self {
            Self::Xfce => "xfce",
            Self::Mde => "mde",
            Self::Pipedal => "pipedal",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "mde" => Self::Mde,
            "pipedal" => Self::Pipedal,
            _ => Self::Xfce,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Xfce => "XFCE",
            Self::Mde => "MDE",
            Self::Pipedal => "PiPedal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileChoice {
    Workstation,
    Minimal,
}
impl ProfileChoice {
    fn as_str(self) -> &'static str {
        match self {
            Self::Workstation => "workstation",
            Self::Minimal => "minimal",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "minimal" => Self::Minimal,
            _ => Self::Workstation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UninstallChoice {
    None,
    Mde,
    Xfce,
    Pipedal,
    All,
}
impl UninstallChoice {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Mde => "mde",
            Self::Xfce => "xfce",
            Self::Pipedal => "pipedal",
            Self::All => "all",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "mde" => Self::Mde,
            "xfce" => Self::Xfce,
            "pipedal" => Self::Pipedal,
            "all" => Self::All,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MasterAction {
    InstallAll,
    KeepInstalled,
    RemoveInstalled,
    NoneAll,
}

#[derive(Debug, Clone)]
enum Msg {
    OpenDesktop,
    OpenUsers,
    OpenCategory(usize),
    BackHub,
    SetDe(DeChoice),
    SetProfile(ProfileChoice),
    SetUninstallDe(UninstallChoice),
    ToggleUser(String, bool),
    ToggleItem(String, bool),
    CategoryMaster(MasterAction),
    Begin,
    ConfirmCatalogRemovals(bool),
    ConfirmDeUninstall(bool),
    Tick,
    Quit,
    Reboot,
}

#[derive(Debug, Serialize)]
struct Choices {
    de: String,
    mde_profile: String,
    users: Vec<String>,
    items: Vec<String>,
    remove_items: Vec<String>,
    uninstall_de: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ApplyResult {
    #[serde(default)]
    applied: i64,
    #[serde(default)]
    skipped: i64,
    #[serde(default)]
    failed: i64,
    #[serde(default)]
    failures: Vec<String>,
    #[serde(default)]
    desktop: String,
    #[serde(default)]
    log: String,
    #[serde(default)]
    mok: String,
}

#[derive(Debug, Clone)]
struct PhaseRow {
    title: String,
    done: bool,
    ok: bool,
    active: bool,
}

struct App {
    model: Model,
    script: PathBuf,
    page: Page,
    de: String,
    mde_profile: String,
    uninstall_de: String,
    users: BTreeSet<String>,
    intents: BTreeMap<String, ItemIntent>,
    present: BTreeMap<String, bool>,
    labels: BTreeMap<String, String>,
    pending_remove_confirm: Option<Vec<String>>,
    pending_de_confirm: Option<String>,
    removes_confirmed: bool,
    de_confirmed: bool,
    child: Option<Child>,
    progress_path: Option<PathBuf>,
    result_path: Option<PathBuf>,
    progress_pos: u64,
    log_pos: u64,
    phase_n: u32,
    phase_total: u32,
    phase_title: String,
    phases: Vec<PhaseRow>,
    current_item: String,
    activity: VecDeque<String>,
    log_tail: VecDeque<String>,
    fraction: f32,
    status: String,
    toast: String,
    finished_ok: Option<bool>,
    apply_result: Option<ApplyResult>,
    pulse: u32,
}

impl App {
    fn new(model: Model, script: PathBuf) -> Self {
        let de = if matches!(model.default_de.as_str(), "xfce" | "mde" | "pipedal") {
            model.default_de.clone()
        } else {
            "xfce".into()
        };
        let users: BTreeSet<String> = model.users.iter().cloned().collect();
        let mut app = Self {
            model,
            script,
            page: Page::Hub,
            de,
            mde_profile: "workstation".into(),
            uninstall_de: String::new(),
            users,
            intents: BTreeMap::new(),
            present: BTreeMap::new(),
            labels: BTreeMap::new(),
            pending_remove_confirm: None,
            pending_de_confirm: None,
            removes_confirmed: false,
            de_confirmed: false,
            child: None,
            progress_path: None,
            result_path: None,
            progress_pos: 0,
            log_pos: 0,
            phase_n: 0,
            phase_total: 11,
            phase_title: "Ready".into(),
            phases: Vec::new(),
            current_item: String::new(),
            activity: VecDeque::new(),
            log_tail: VecDeque::new(),
            fraction: 0.0,
            status: String::new(),
            toast: String::new(),
            finished_ok: None,
            apply_result: None,
            pulse: 0,
        };
        if !app.model.mde_profile.is_empty() {
            app.mde_profile = app.model.mde_profile.clone();
        }
        app.rebuild_intents();
        app.push_activity("Review the summary, then begin when ready.");
        app
    }

    fn push_activity(&mut self, line: impl Into<String>) {
        self.activity.push_back(line.into());
        while self.activity.len() > 12 {
            self.activity.pop_front();
        }
    }

    fn cats(&self) -> &[Category] {
        self.model
            .categories
            .get(&self.de)
            .map(|c| c.as_slice())
            .unwrap_or(&[])
    }

    fn rebuild_intents(&mut self) {
        let cats = self.cats().to_vec();
        self.intents.clear();
        self.present.clear();
        self.labels.clear();
        for cat in cats {
            for item in cat.items {
                let present = item.present
                    || item.label.contains("[keep]")
                    || item.label.contains("[installed]");
                self.present.insert(item.id.clone(), present);
                let base = item
                    .label
                    .replace(" [keep]", "")
                    .replace(" [install]", "")
                    .replace(" [remove]", "")
                    .replace(" [installed]", "");
                self.labels.insert(item.id.clone(), base);
                let intent = if present {
                    if item.on {
                        ItemIntent::Keep
                    } else {
                        ItemIntent::Remove
                    }
                } else if item.on {
                    ItemIntent::Install
                } else {
                    ItemIntent::Leave
                };
                self.intents.insert(item.id.clone(), intent);
            }
        }
    }

    fn checked(intent: ItemIntent) -> bool {
        matches!(intent, ItemIntent::Install | ItemIntent::Keep)
    }

    fn set_toggle(&mut self, id: &str, on: bool) {
        let present = *self.present.get(id).unwrap_or(&false);
        let intent = if present {
            if on {
                ItemIntent::Keep
            } else {
                ItemIntent::Remove
            }
        } else if on {
            ItemIntent::Install
        } else {
            ItemIntent::Leave
        };
        let label = self.labels.get(id).cloned().unwrap_or_else(|| id.to_string());
        let (badge, _) = intent.badge();
        self.push_activity(format!("{label} → {badge}"));
        self.toast = format!("{label}: {badge}");
        self.intents.insert(id.to_string(), intent);
    }

    fn apply_master(&mut self, cat_idx: usize, action: MasterAction) {
        let Some(cat) = self.cats().get(cat_idx).cloned() else {
            return;
        };
        let name = match action {
            MasterAction::InstallAll => "install all",
            MasterAction::KeepInstalled => "keep installed",
            MasterAction::RemoveInstalled => "remove installed",
            MasterAction::NoneAll => "clear all",
        };
        self.push_activity(format!("{}: {name}", cat.title));
        self.toast = format!("{} — {name}", cat.title);
        for item in cat.items {
            let present = *self.present.get(&item.id).unwrap_or(&false);
            let intent = match action {
                MasterAction::InstallAll => ItemIntent::Install,
                MasterAction::KeepInstalled => {
                    if present {
                        ItemIntent::Keep
                    } else {
                        ItemIntent::Leave
                    }
                }
                MasterAction::RemoveInstalled => {
                    if present {
                        ItemIntent::Remove
                    } else {
                        ItemIntent::Leave
                    }
                }
                MasterAction::NoneAll => ItemIntent::Leave,
            };
            self.intents.insert(item.id, intent);
        }
    }

    fn build_choices(&self) -> Choices {
        let mut items = Vec::new();
        let mut remove_items = Vec::new();
        for (id, intent) in &self.intents {
            match intent {
                ItemIntent::Install => items.push(id.clone()),
                ItemIntent::Remove => remove_items.push(id.clone()),
                ItemIntent::Keep | ItemIntent::Leave => {}
            }
        }
        items.sort();
        remove_items.sort();
        let mut users: Vec<String> = self.users.iter().cloned().collect();
        users.sort();
        Choices {
            de: self.de.clone(),
            mde_profile: self.mde_profile.clone(),
            users,
            items,
            remove_items,
            uninstall_de: self.uninstall_de.clone(),
        }
    }

    fn totals(&self) -> (usize, usize, usize) {
        let mut install = 0;
        let mut keep = 0;
        let mut remove = 0;
        for intent in self.intents.values() {
            match intent {
                ItemIntent::Install => install += 1,
                ItemIntent::Keep => keep += 1,
                ItemIntent::Remove => remove += 1,
                ItemIntent::Leave => {}
            }
        }
        (install, keep, remove)
    }

    fn hub_status_cat(&self, idx: usize) -> (usize, usize, usize, usize) {
        let Some(cat) = self.cats().get(idx) else {
            return (0, 0, 0, 0);
        };
        let mut install = 0;
        let mut keep = 0;
        let mut remove = 0;
        for item in &cat.items {
            match self.intents.get(&item.id).copied().unwrap_or(ItemIntent::Leave) {
                ItemIntent::Install => install += 1,
                ItemIntent::Keep => keep += 1,
                ItemIntent::Remove => remove += 1,
                ItemIntent::Leave => {}
            }
        }
        (install, keep, remove, cat.items.len())
    }

    fn start_apply(&mut self) -> Task<Msg> {
        let choices = self.build_choices();
        if !choices.remove_items.is_empty() && !self.removes_confirmed {
            self.pending_remove_confirm = Some(choices.remove_items.clone());
            return Task::none();
        }
        if !choices.uninstall_de.is_empty() && !self.de_confirmed {
            self.pending_de_confirm = Some(choices.uninstall_de.clone());
            return Task::none();
        }
        self.pending_remove_confirm = None;
        self.pending_de_confirm = None;

        let work = PathBuf::from(&self.model.log)
            .parent()
            .unwrap_or(Path::new("/tmp"))
            .join(format!(".ui-{}", std::process::id()));
        let _ = fs::create_dir_all(&work);
        let choices_path = work.join("choices.json");
        let progress_path = work.join("progress");
        let result_path = work.join("result.json");
        let _ = fs::write(&progress_path, "");
        if let Err(e) = fs::write(
            &choices_path,
            serde_json::to_string_pretty(&choices).unwrap_or_default(),
        ) {
            self.status = format!("Failed to write choices: {e}");
            self.toast = self.status.clone();
            return Task::none();
        }

        // Snapshot starting log offset so we only show new lines.
        self.log_pos = fs::metadata(&self.model.log)
            .map(|m| m.len())
            .unwrap_or(0);
        self.progress_pos = 0;
        self.fraction = 0.0;
        self.phase_n = 0;
        self.phase_total = 11;
        self.phase_title = "Starting…".into();
        self.phases.clear();
        self.current_item.clear();
        self.log_tail.clear();
        self.apply_result = None;
        self.finished_ok = None;

        let mut cmd = Command::new(&self.script);
        cmd.arg(format!("--apply-choices={}", choices_path.display()));
        if self.model.dry_run {
            cmd.arg("--dry-run");
        }
        if self.model.no_surface {
            cmd.arg("--no-surface");
        }
        if !self.model.force_surface.is_empty() {
            cmd.arg(format!("--force-surface={}", self.model.force_surface));
        }
        if !self.model.simulate_surface.is_empty() {
            cmd.arg(format!(
                "--simulate-surface={}",
                self.model.simulate_surface
            ));
        }
        cmd.env("WS_LOG_FILE", &self.model.log);
        cmd.env("WS_LOG_QUIET", "1");
        cmd.env("WS_PROGRESS_FILE", &progress_path);
        cmd.env("WS_RESULT_FILE", &result_path);
        cmd.stdin(Stdio::null());
        let logf = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.model.log);
        match logf {
            Ok(f) => {
                cmd.stdout(Stdio::from(f.try_clone().unwrap()));
                cmd.stderr(Stdio::from(f));
            }
            Err(e) => {
                self.status = format!("Failed to open log: {e}");
                self.toast = self.status.clone();
                return Task::none();
            }
        }

        match cmd.spawn() {
            Ok(child) => {
                info!(pid = child.id(), "apply started");
                self.child = Some(child);
                self.progress_path = Some(progress_path);
                self.result_path = Some(result_path);
                self.page = Page::Progress;
                self.status = if self.model.dry_run {
                    "Dry run in progress…".into()
                } else {
                    "Applying changes…".into()
                };
                self.toast = self.status.clone();
                self.push_activity(format!(
                    "Started apply (install {} · remove {} · uninstall {})",
                    choices.items.len(),
                    choices.remove_items.len(),
                    if choices.uninstall_de.is_empty() {
                        "none"
                    } else {
                        &choices.uninstall_de
                    }
                ));
                tick_soon()
            }
            Err(e) => {
                self.status = format!("Failed to start apply: {e}");
                self.toast = self.status.clone();
                Task::none()
            }
        }
    }

    fn read_progress(&mut self) {
        let Some(path) = self.progress_path.clone() else {
            return;
        };
        let Ok(mut f) = File::open(&path) else {
            return;
        };
        let _ = f.seek(SeekFrom::Start(self.progress_pos));
        let mut buf = String::new();
        if f.read_to_string(&mut buf).is_err() {
            return;
        }
        self.progress_pos += buf.len() as u64;
        for line in buf.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }
            match parts[0] {
                "PHASE" if parts.len() >= 4 => {
                    let n: u32 = parts[1].parse().unwrap_or(1);
                    let total: u32 = parts[2].parse().unwrap_or(1).max(1);
                    let title = parts[3].to_string();
                    self.phase_n = n;
                    self.phase_total = total;
                    self.phase_title = title.clone();
                    self.fraction = ((n.saturating_sub(1)) as f32 / total as f32).clamp(0.0, 1.0);
                    for p in &mut self.phases {
                        p.active = false;
                    }
                    if let Some(existing) = self.phases.iter_mut().find(|p| p.title == title) {
                        existing.active = true;
                        existing.done = false;
                    } else {
                        self.phases.push(PhaseRow {
                            title: title.clone(),
                            done: false,
                            ok: true,
                            active: true,
                        });
                    }
                    self.push_activity(format!("Phase {n}/{total}: {title}"));
                    self.status = title;
                }
                "DONE" if parts.len() >= 3 => {
                    let ok = parts[1] != "0";
                    let title = parts[2];
                    if let Some(p) = self.phases.iter_mut().rev().find(|p| p.title == title) {
                        p.done = true;
                        p.ok = ok;
                        p.active = false;
                    }
                    let done = self.phases.iter().filter(|p| p.done).count() as f32;
                    self.fraction = (done / self.phase_total.max(1) as f32).clamp(0.0, 1.0);
                    self.push_activity(format!(
                        "{} {}",
                        if ok { "✓" } else { "✗" },
                        title
                    ));
                }
                "INFO" if parts.len() >= 2 => {
                    let msg = parts[1..].join("\t");
                    self.current_item = msg.clone();
                    self.push_activity(msg);
                }
                "ITEM" if parts.len() >= 4 => {
                    let op = parts[1];
                    let label = parts[3..].join("\t");
                    let verb = if op == "remove" { "Removing" } else { "Installing" };
                    let msg = format!("{verb}: {label}");
                    self.current_item = msg.clone();
                    self.push_activity(msg);
                }
                "ITEM_DONE" if parts.len() >= 3 => {
                    let ok = parts[2] != "0";
                    if !self.current_item.is_empty() {
                        self.push_activity(format!(
                            "{} {}",
                            if ok { "✓" } else { "✗" },
                            self.current_item
                        ));
                    }
                    self.current_item.clear();
                }
                _ => {}
            }
        }
    }

    fn read_log(&mut self) {
        let Ok(mut f) = File::open(&self.model.log) else {
            return;
        };
        let _ = f.seek(SeekFrom::Start(self.log_pos));
        let mut buf = String::new();
        if f.read_to_string(&mut buf).is_err() {
            return;
        }
        self.log_pos += buf.len() as u64;
        for line in buf.lines() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            // Prefer concise RUN/DNF/Catalog lines for the live feed.
            if trimmed.contains("INFO")
                || trimmed.contains("WARN")
                || trimmed.contains("ERROR")
                || trimmed.contains("DRY-RUN")
                || trimmed.contains("Catalog ")
                || trimmed.contains("APPLY")
            {
                self.log_tail.push_back(trimmed.to_string());
            }
            while self.log_tail.len() > 80 {
                self.log_tail.pop_front();
            }
        }
    }

    fn poll_child(&mut self) -> Task<Msg> {
        self.pulse = self.pulse.wrapping_add(1);
        self.read_progress();
        self.read_log();
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.child = None;
                    self.read_progress();
                    self.read_log();
                    self.fraction = 1.0;
                    let ok = status.success();
                    self.finished_ok = Some(ok);
                    if let Some(path) = &self.result_path {
                        if let Ok(raw) = fs::read_to_string(path) {
                            self.apply_result = serde_json::from_str(&raw).ok();
                        }
                    }
                    self.page = Page::Done;
                    self.phase_title = if ok {
                        "Complete".into()
                    } else {
                        "Finished with problems".into()
                    };
                    self.status = self.phase_title.clone();
                    self.toast = self.status.clone();
                    self.push_activity(if ok {
                        "Apply finished successfully.".into()
                    } else {
                        format!("Apply exited: {status}")
                    });
                    return Task::none();
                }
                Ok(None) => return tick_soon(),
                Err(e) => {
                    warn!(error = %e, "wait failed");
                    self.child = None;
                    self.page = Page::Done;
                    self.finished_ok = Some(false);
                    self.status = format!("Apply wait error: {e}");
                    self.toast = self.status.clone();
                }
            }
        }
        Task::none()
    }

    // --- view helpers ----------------------------------------------------

    fn shell<'a>(&'a self, body: Element<'a, Msg>) -> Element<'a, Msg> {
        let brand = row![
            text("MACKES").size(14).color(ACCENT),
            Space::with_width(10),
            text("Fedora Workstation Setup").size(14).color(MUTED),
            Space::with_width(Length::Fill),
            if self.model.dry_run {
                badge("DRY RUN", WARN)
            } else {
                badge("LIVE", OK)
            },
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let toast: Element<'_, Msg> = if self.toast.is_empty() {
            Space::with_height(0).into()
        } else {
            container(text(&self.toast).size(13).color(MUTED))
                .padding(Padding::from([6, 12]))
                .width(Length::Fill)
                .style(|_| chip_style(SURFACE2))
                .into()
        };

        let footer = match self.page {
            Page::Hub => row![
                ghost_btn("Quit", Msg::Quit),
                Space::with_width(Length::Fill),
                primary_btn("Begin Installation", Msg::Begin),
            ]
            .spacing(12),
            Page::Desktop | Page::Users | Page::Category(_) => {
                row![ghost_btn("Back", Msg::BackHub), Space::with_width(Length::Fill),].spacing(12)
            }
            Page::Progress => row![
                text(format!(
                    "Step {}/{} · {}",
                    self.phase_n.max(1),
                    self.phase_total,
                    self.phase_title
                ))
                .size(13)
                .color(MUTED),
                Space::with_width(Length::Fill),
            ],
            Page::Done => {
                let mut r = row![ghost_btn("Quit", Msg::Quit)].spacing(12);
                if self.finished_ok == Some(true) && !self.model.dry_run {
                    r = r.push(primary_btn("Reboot", Msg::Reboot));
                }
                r.push(Space::with_width(Length::Fill))
            }
        };

        container(
            column![
                container(brand)
                    .padding(Padding::from([16, 28]))
                    .width(Length::Fill)
                    .style(|_| bar_style()),
                container(body)
                    .padding(Padding::from([20, 28]))
                    .width(Length::Fill)
                    .height(Length::Fill),
                container(column![toast, footer].spacing(8))
                    .padding(Padding::from([12, 28]))
                    .width(Length::Fill)
                    .style(|_| bar_style()),
            ]
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(BG)),
            text_color: Some(TEXT),
            ..Default::default()
        })
        .into()
    }

    fn view_hub(&self) -> Element<'_, Msg> {
        let (install, keep, remove) = self.totals();
        let summary = row![
            stat_chip("Install", install, INSTALL_C),
            stat_chip("Keep", keep, KEEP_C),
            stat_chip("Remove", remove, REMOVE_C),
            Space::with_width(Length::Fill),
            text(format!("Desktop · {}", DeChoice::from_str(&self.de).label()))
                .size(14)
                .color(MUTED),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        let mut spokes = column![
            spoke_card(
                "Desktop",
                format!(
                    "{}{}",
                    DeChoice::from_str(&self.de).label(),
                    if self.uninstall_de.is_empty() {
                        String::new()
                    } else {
                        format!(" · uninstall {}", self.uninstall_de)
                    }
                ),
                Msg::OpenDesktop,
            ),
            spoke_card(
                "Users",
                format!("{} selected for look", self.users.len()),
                Msg::OpenUsers,
            ),
        ]
        .spacing(10);

        for (i, cat) in self.cats().iter().enumerate() {
            let (ins, kp, rm, total) = self.hub_status_cat(i);
            spokes = spokes.push(spoke_card(
                &cat.title,
                format!("{ins} install · {kp} keep · {rm} remove / {total}"),
                Msg::OpenCategory(i),
            ));
        }

        let activity = activity_panel(&self.activity);

        self.shell(
            column![
                text("Installation summary").size(28),
                text("Checked items install or stay. Uncheck an installed item to remove it.")
                    .size(14)
                    .color(MUTED),
                Space::with_height(12),
                summary,
                Space::with_height(16),
                row![
                    scrollable(spokes).height(Length::Fill).width(Length::FillPortion(3)),
                    Space::with_width(16),
                    container(activity)
                        .width(Length::FillPortion(2))
                        .height(Length::Fill),
                ]
                .height(Length::Fill),
            ]
            .spacing(6)
            .height(Length::Fill)
            .into(),
        )
    }

    fn view_desktop(&self) -> Element<'_, Msg> {
        let de = DeChoice::from_str(&self.de);
        let mut body = column![
            text("Desktop").size(28),
            text("Choose the target desktop. Optionally uninstall an experience already on this machine.")
                .size(14)
                .color(MUTED),
            Space::with_height(12),
            section_label("Target"),
            radio("XFCE — classic panel desktop", DeChoice::Xfce, Some(de), Msg::SetDe),
            radio("MDE — Mackes Desktop on labwc", DeChoice::Mde, Some(de), Msg::SetDe),
            radio("PiPedal — console appliance", DeChoice::Pipedal, Some(de), Msg::SetDe),
        ]
        .spacing(8);

        if self.de == "mde" {
            let profile = ProfileChoice::from_str(&self.mde_profile);
            body = body
                .push(Space::with_height(8))
                .push(section_label("MDE profile"))
                .push(radio(
                    "Workstation — shell + baseline",
                    ProfileChoice::Workstation,
                    Some(profile),
                    Msg::SetProfile,
                ))
                .push(radio(
                    "Minimal — compositor and shell only",
                    ProfileChoice::Minimal,
                    Some(profile),
                    Msg::SetProfile,
                ));
        }

        let uninstall = UninstallChoice::from_str(&self.uninstall_de);
        body = body
            .push(Space::with_height(12))
            .push(section_label("Uninstall experience"))
            .push(radio(
                "Do not uninstall",
                UninstallChoice::None,
                Some(uninstall),
                Msg::SetUninstallDe,
            ))
            .push(radio(
                "Uninstall MDE",
                UninstallChoice::Mde,
                Some(uninstall),
                Msg::SetUninstallDe,
            ))
            .push(radio(
                "Uninstall XFCE",
                UninstallChoice::Xfce,
                Some(uninstall),
                Msg::SetUninstallDe,
            ))
            .push(radio(
                "Uninstall PiPedal",
                UninstallChoice::Pipedal,
                Some(uninstall),
                Msg::SetUninstallDe,
            ))
            .push(radio(
                "Uninstall all managed experiences",
                UninstallChoice::All,
                Some(uninstall),
                Msg::SetUninstallDe,
            ));

        self.shell(scrollable(body).height(Length::Fill).into())
    }

    fn view_users(&self) -> Element<'_, Msg> {
        let mut body = column![
            text("Users").size(28),
            text("Apply the look for these local accounts.").size(14).color(MUTED),
            Space::with_height(12),
        ]
        .spacing(8);
        for u in &self.model.users {
            let on = self.users.contains(u);
            body = body.push(
                container(
                    checkbox(u.clone(), on).on_toggle({
                        let u = u.clone();
                        move |v| Msg::ToggleUser(u.clone(), v)
                    }),
                )
                .padding(12)
                .width(Length::Fill)
                .style(|_| card_style()),
            );
        }
        if self.model.users.is_empty() {
            body = body.push(text("No local users found.").color(MUTED));
        }
        self.shell(scrollable(body).height(Length::Fill).into())
    }

    fn view_category(&self, idx: usize) -> Element<'_, Msg> {
        let cat = match self.cats().get(idx) {
            Some(c) => c,
            None => return self.shell(text("Missing category").into()),
        };
        let mut body = column![
            text(cat.title.clone()).size(28),
            text("Checked = install or keep. Uncheck installed = remove. Unchecked absent = leave.")
                .size(14)
                .color(MUTED),
            Space::with_height(10),
            row![
                master_btn("Install all", Msg::CategoryMaster(MasterAction::InstallAll)),
                master_btn("Keep installed", Msg::CategoryMaster(MasterAction::KeepInstalled)),
                master_btn(
                    "Remove installed",
                    Msg::CategoryMaster(MasterAction::RemoveInstalled)
                ),
                master_btn("None", Msg::CategoryMaster(MasterAction::NoneAll)),
            ]
            .spacing(8),
            Space::with_height(8),
        ]
        .spacing(6);

        for item in &cat.items {
            let intent = self
                .intents
                .get(&item.id)
                .copied()
                .unwrap_or(ItemIntent::Leave);
            let base = self
                .labels
                .get(&item.id)
                .cloned()
                .unwrap_or_else(|| item.label.clone());
            let (badge_txt, badge_c) = intent.badge();
            let id = item.id.clone();
            body = body.push(
                container(
                    row![
                        checkbox(base, Self::checked(intent))
                            .on_toggle(move |v| Msg::ToggleItem(id.clone(), v))
                            .width(Length::Fill),
                        badge(badge_txt, badge_c),
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Center),
                )
                .padding(Padding::from([10, 14]))
                .width(Length::Fill)
                .style(|_| card_style()),
            );
        }

        self.shell(scrollable(body).height(Length::Fill).into())
    }

    fn view_progress(&self) -> Element<'_, Msg> {
        let bar = progress_bar(0.0..=1.0, self.fraction)
            .style(|_| progress_bar::Style {
                background: Background::Color(SURFACE2),
                bar: Background::Color(ACCENT),
                border: Border {
                    color: BORDER,
                    width: 0.0,
                    radius: 4.0.into(),
                },
            })
            .height(10);

        let pct = (self.fraction * 100.0).round() as u32;
        let header = column![
            text(if self.model.dry_run {
                "Dry-run progress"
            } else {
                "Installation progress"
            })
            .size(28),
            text(&self.phase_title).size(18).color(ACCENT),
            if self.current_item.is_empty() {
                text(" ").size(13)
            } else {
                text(&self.current_item).size(13).color(MUTED)
            },
            Space::with_height(8),
            row![
                bar.width(Length::Fill),
                text(format!("{pct}%")).size(14).color(MUTED),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(4);

        let mut phase_col = column![section_label("Phases")].spacing(4);
        for p in &self.phases {
            let mark = if p.active {
                "▶"
            } else if p.done && p.ok {
                "✓"
            } else if p.done {
                "✗"
            } else {
                "·"
            };
            let c = if p.active {
                ACCENT
            } else if p.done && p.ok {
                OK
            } else if p.done {
                DANGER
            } else {
                MUTED
            };
            phase_col = phase_col.push(
                text(format!("{mark}  {}", p.title)).size(13).color(c),
            );
        }
        if self.phases.is_empty() {
            phase_col = phase_col.push(text("Waiting for first phase…").size(13).color(MUTED));
        }

        let mut log_col = column![section_label("Live log")].spacing(2);
        for line in self.log_tail.iter().rev().take(40).collect::<Vec<_>>().into_iter().rev()
        {
            let c = if line.contains("ERROR") {
                DANGER
            } else if line.contains("WARN") {
                WARN
            } else if line.contains("Catalog remove") || line.contains("DRY-RUN: [remove") {
                REMOVE_C
            } else if line.contains("Catalog apply") || line.contains("install") {
                INSTALL_C
            } else {
                MUTED
            };
            log_col = log_col.push(text(line).size(12).font(Font::MONOSPACE).color(c));
        }
        if self.log_tail.is_empty() {
            log_col = log_col.push(text("Waiting for log output…").size(12).color(MUTED));
        }

        let pulse = if self.pulse % 2 == 0 { "●" } else { "○" };

        self.shell(
            column![
                header,
                Space::with_height(8),
                text(format!("{pulse} Working…")).size(12).color(MUTED),
                Space::with_height(12),
                row![
                    container(scrollable(phase_col).height(Length::Fill))
                        .padding(14)
                        .width(Length::FillPortion(2))
                        .height(Length::Fill)
                        .style(|_| card_style()),
                    Space::with_width(12),
                    container(scrollable(log_col).height(Length::Fill))
                        .padding(14)
                        .width(Length::FillPortion(3))
                        .height(Length::Fill)
                        .style(|_| card_style()),
                ]
                .height(Length::Fill),
            ]
            .spacing(4)
            .height(Length::Fill)
            .into(),
        )
    }

    fn view_done(&self) -> Element<'_, Msg> {
        let ok = self.finished_ok.unwrap_or(false);
        let title = if ok { "Setup complete" } else { "Finished with problems" };
        let accent = if ok { OK } else { DANGER };

        let mut stats = column![].spacing(6);
        if let Some(r) = &self.apply_result {
            stats = stats
                .push(text(format!("Desktop: {}", r.desktop)).size(14))
                .push(text(format!(
                    "Applied {} · Skipped {} · Failed {}",
                    r.applied, r.skipped, r.failed
                )).size(14))
                .push(
                    text(if self.model.dry_run {
                        "Dry run — no system changes were made."
                    } else {
                        "Changes were applied to this system."
                    })
                    .size(13)
                    .color(MUTED),
                );
            if !r.failures.is_empty() {
                stats = stats.push(
                    text(format!("Failures: {}", r.failures.join(", ")))
                        .size(13)
                        .color(DANGER),
                );
            }
            if !r.mok.trim().is_empty() {
                stats = stats.push(text(r.mok.trim()).size(13).color(WARN));
            }
            let log = if r.log.is_empty() {
                self.model.log.clone()
            } else {
                r.log.clone()
            };
            stats = stats.push(text(format!("Log: {log}")).size(12).color(MUTED));
        } else {
            stats = stats.push(text(&self.status).size(14));
        }

        let mut act = column![section_label("Recent activity")].spacing(3);
        for line in self.activity.iter().rev().take(8).collect::<Vec<_>>().into_iter().rev() {
            act = act.push(text(line).size(12).color(MUTED));
        }

        self.shell(
            column![
                text(title).size(28).color(accent),
                Space::with_height(8),
                container(stats)
                    .padding(16)
                    .width(Length::Fill)
                    .style(|_| card_style()),
                Space::with_height(12),
                container(act)
                    .padding(16)
                    .width(Length::Fill)
                    .style(|_| card_style()),
            ]
            .spacing(4)
            .into(),
        )
    }

    fn view_confirm(&self) -> Element<'_, Msg> {
        if let Some(list) = &self.pending_remove_confirm {
            let joined = list.join(", ");
            return modal(
                "Confirm catalog removals",
                format!("These items will be removed:\n\n{joined}"),
                "Back",
                Msg::ConfirmCatalogRemovals(false),
                "Remove and continue",
                Msg::ConfirmCatalogRemovals(true),
                true,
            );
        }
        if let Some(de) = &self.pending_de_confirm {
            return modal(
                "Confirm desktop uninstall",
                format!(
                    "This uninstalls the {de} experience.\nShared keep-apps stay installed."
                ),
                "Back",
                Msg::ConfirmDeUninstall(false),
                "Uninstall and continue",
                Msg::ConfirmDeUninstall(true),
                true,
            );
        }
        text("").into()
    }
}

impl App {
    fn title(&self) -> String {
        "Mackes Fedora Linux Setup".into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::OpenDesktop => self.page = Page::Desktop,
            Msg::OpenUsers => self.page = Page::Users,
            Msg::OpenCategory(i) => self.page = Page::Category(i),
            Msg::BackHub => {
                self.page = Page::Hub;
                self.toast.clear();
            }
            Msg::SetDe(de) => {
                let next = de.as_str().to_string();
                if next != self.de {
                    self.de = next;
                    self.rebuild_intents();
                    self.toast = format!("Desktop → {}", de.label());
                    self.push_activity(self.toast.clone());
                }
            }
            Msg::SetProfile(p) => {
                self.mde_profile = p.as_str().to_string();
                self.toast = format!("MDE profile → {}", p.as_str());
            }
            Msg::SetUninstallDe(v) => {
                self.uninstall_de = v.as_str().to_string();
                self.de_confirmed = false;
                self.toast = if self.uninstall_de.is_empty() {
                    "No desktop uninstall queued".into()
                } else {
                    format!("Will uninstall {}", self.uninstall_de)
                };
                self.push_activity(self.toast.clone());
            }
            Msg::ToggleUser(u, on) => {
                if on {
                    self.users.insert(u.clone());
                    self.toast = format!("User {u} selected");
                } else {
                    self.users.remove(&u);
                    self.toast = format!("User {u} deselected");
                }
            }
            Msg::ToggleItem(id, on) => {
                self.removes_confirmed = false;
                self.set_toggle(&id, on);
            }
            Msg::CategoryMaster(action) => {
                self.removes_confirmed = false;
                if let Page::Category(i) = self.page {
                    self.apply_master(i, action);
                }
            }
            Msg::Begin => return self.start_apply(),
            Msg::ConfirmCatalogRemovals(ok) => {
                self.pending_remove_confirm = None;
                if ok {
                    self.removes_confirmed = true;
                    self.push_activity("Catalog removals confirmed");
                    return self.start_apply();
                }
                self.removes_confirmed = false;
                self.toast = "Removal cancelled".into();
            }
            Msg::ConfirmDeUninstall(ok) => {
                self.pending_de_confirm = None;
                if ok {
                    self.de_confirmed = true;
                    self.push_activity("Desktop uninstall confirmed");
                    return self.start_apply();
                }
                self.de_confirmed = false;
                self.toast = "Desktop uninstall cancelled".into();
            }
            Msg::Tick => return self.poll_child(),
            Msg::Quit => {
                if let Some(mut c) = self.child.take() {
                    let _ = c.kill();
                }
                std::process::exit(0);
            }
            Msg::Reboot => {
                let _ = Command::new("systemctl").arg("reboot").spawn();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Msg> {
        if self.pending_remove_confirm.is_some() || self.pending_de_confirm.is_some() {
            return self.view_confirm();
        }
        match self.page {
            Page::Hub => self.view_hub(),
            Page::Desktop => self.view_desktop(),
            Page::Users => self.view_users(),
            Page::Category(i) => self.view_category(i),
            Page::Progress => self.view_progress(),
            Page::Done => self.view_done(),
        }
    }
}

fn tick_soon() -> Task<Msg> {
    Task::perform(
        async {
            tokio::time::sleep(Duration::from_millis(250)).await;
        },
        |_| Msg::Tick,
    )
}

// --- styled widgets ------------------------------------------------------

fn badge<'a>(label: &'a str, color: Color) -> Element<'a, Msg> {
    container(text(label).size(11).color(color))
        .padding(Padding::from([3, 8]))
        .style(move |_| chip_style_color(color))
        .into()
}

fn stat_chip<'a>(label: &'a str, n: usize, color: Color) -> Element<'a, Msg> {
    container(
        row![
            text(format!("{n}")).size(18).color(color),
            text(label).size(12).color(MUTED),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding(Padding::from([8, 12]))
    .style(|_| card_style())
    .into()
}

fn section_label<'a>(s: &'a str) -> Element<'a, Msg> {
    text(s.to_uppercase()).size(11).color(MUTED).into()
}

fn spoke_card<'a>(title: &'a str, status: String, msg: Msg) -> Element<'a, Msg> {
    button(
        row![
            column![
                text(title).size(16),
                text(status).size(12).color(MUTED),
            ]
            .spacing(2)
            .width(Length::Fill),
            text("›").size(22).color(MUTED),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding(Padding::from([14, 16])),
    )
    .on_press(msg)
    .width(Length::Fill)
    .style(|_, status| spoke_btn_style(status))
    .into()
}

fn activity_panel<'a>(lines: &'a VecDeque<String>) -> Element<'a, Msg> {
    let mut col = column![section_label("Activity")].spacing(4);
    if lines.is_empty() {
        col = col.push(text("No changes yet.").size(12).color(MUTED));
    } else {
        for line in lines.iter().rev().take(10).collect::<Vec<_>>().into_iter().rev() {
            col = col.push(text(line).size(12).color(MUTED));
        }
    }
    container(col)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| card_style())
        .into()
}

fn primary_btn<'a>(label: &'a str, msg: Msg) -> Element<'a, Msg> {
    button(text(label).size(14))
        .padding(Padding::from([10, 18]))
        .on_press(msg)
        .style(|_, status| primary_style(status))
        .into()
}

fn ghost_btn<'a>(label: &'a str, msg: Msg) -> Element<'a, Msg> {
    button(text(label).size(14))
        .padding(Padding::from([10, 18]))
        .on_press(msg)
        .style(|_, status| ghost_style(status))
        .into()
}

fn master_btn<'a>(label: &'a str, msg: Msg) -> Element<'a, Msg> {
    button(text(label).size(12))
        .padding(Padding::from([6, 10]))
        .on_press(msg)
        .style(|_, status| ghost_style(status))
        .into()
}

fn modal<'a>(
    title: &'a str,
    body: String,
    back: &'a str,
    back_msg: Msg,
    go: &'a str,
    go_msg: Msg,
    danger: bool,
) -> Element<'a, Msg> {
    container(
        container(
            column![
                text(title).size(22),
                text(body).size(14).color(MUTED),
                Space::with_height(16),
                row![
                    ghost_btn(back, back_msg),
                    Space::with_width(Length::Fill),
                    if danger {
                        button(text(go).size(14))
                            .padding(Padding::from([10, 18]))
                            .on_press(go_msg)
                            .style(|_, status| danger_style(status))
                            .into()
                    } else {
                        primary_btn(go, go_msg)
                    },
                ]
                .spacing(12),
            ]
            .spacing(10)
            .padding(28)
            .max_width(560),
        )
        .style(|_| card_style()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(Color {
            a: 0.72,
            ..BG
        })),
        ..Default::default()
    })
    .into()
}

fn bar_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: BORDER,
            width: 0.0,
            radius: Radius::default(),
        },
        ..Default::default()
    }
}

fn card_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

fn chip_style(bg: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn chip_style_color(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color {
            a: 0.15,
            ..color
        })),
        border: Border {
            color: Color { a: 0.45, ..color },
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn spoke_btn_style(status: iced::widget::button::Status) -> button::Style {
    let hovered = matches!(status, iced::widget::button::Status::Hovered);
    button::Style {
        background: Some(Background::Color(if hovered { SURFACE2 } else { SURFACE })),
        text_color: TEXT,
        border: Border {
            color: if hovered { ACCENT } else { BORDER },
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
    }
}

fn primary_style(status: iced::widget::button::Status) -> button::Style {
    let hovered = matches!(status, iced::widget::button::Status::Hovered);
    button::Style {
        background: Some(Background::Color(if hovered { ACCENT_HOVER } else { ACCENT })),
        text_color: Color::WHITE,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        shadow: Shadow::default(),
    }
}

fn ghost_style(status: iced::widget::button::Status) -> button::Style {
    let hovered = matches!(status, iced::widget::button::Status::Hovered);
    button::Style {
        background: Some(Background::Color(if hovered { SURFACE2 } else { SURFACE })),
        text_color: TEXT,
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow: Shadow::default(),
    }
}

fn danger_style(status: iced::widget::button::Status) -> button::Style {
    let hovered = matches!(status, iced::widget::button::Status::Hovered);
    button::Style {
        background: Some(Background::Color(if hovered {
            Color::from_rgb(0.95, 0.42, 0.38)
        } else {
            DANGER
        })),
        text_color: Color::WHITE,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        shadow: Shadow::default(),
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let args = Args::parse();
    let raw = fs::read_to_string(&args.model)
        .with_context(|| format!("read model {}", args.model.display()))?;
    let model: Model = serde_json::from_str(&raw).context("parse model JSON")?;
    if !args.script.is_file() {
        bail!("script not found: {}", args.script.display());
    }

    let app = App::new(model, args.script);
    iced::application(App::title, App::update, App::view)
        .theme(App::theme)
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 760.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            decorations: true,
            ..Default::default()
        })
        .run_with(move || (app, Task::none()))
        .map_err(|e| anyhow::anyhow!("iced: {e}"))?;
    Ok(())
}
