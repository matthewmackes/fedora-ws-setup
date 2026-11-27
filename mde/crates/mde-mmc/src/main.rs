//! mde-mmc — Management Console host (W450+).
//!
//! Usage: mde-mmc [compmgmt.msc|services.msc|…|path.msc|Workstation]
//!
//! UI polish passes (see docs/WIN11_UI_AUDIT.md): icons, columns, menubar,
//! icon toolbar, dynamic title, soft selection, dialog cards, tree expand.

mod bridge;
mod chrome;
mod console;
mod helper;
mod jobs;
mod model;
mod snapin;

use std::collections::HashSet;

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text_input, Space,
};
use iced::{Element, Fill, Length, Padding, Task, Theme};
use mde_theme::Palette;

use crate::chrome::ToolKind;
use crate::jobs::JobBoard;
use crate::model::{ActionItem, ConsoleSpec, ResultsView, TreeNode};

fn main() -> iced::Result {
    mde_config::supervisor::install_panic_hook();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let arg = std::env::args().nth(1);
    let spec = console::resolve_arg(arg.as_deref());
    let icon = resolve_window_icon(&spec);

    let boot_cfg = mde_config::MdeConfig::load().unwrap_or_default();
    let ui = mde_theme::ui_font(&boot_cfg.active().theme).to_string();
    let mono = mde_theme::mono_font(&boot_cfg.active().theme).to_string();
    let mut app = iced::application(Mmc::window_title, Mmc::update, Mmc::view)
        .theme(Mmc::theme)
        .subscription(Mmc::subscription)
        .default_font(iced::Font::with_name(Box::leak(ui.clone().into_boxed_str())))
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 760.0),
            decorations: true,
            icon,
            ..Default::default()
        });
    for bytes in mde_theme::font_bytes_selected(&ui, &mono) {
        app = app.font(bytes);
    }
    app.run_with(move || Mmc::new(spec))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuId {
    File,
    Action,
    View,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragPane {
    Tree,
    Actions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContextTarget {
    Tree(String),
    Row(String),
}

#[derive(Debug, Clone)]
enum Message {
    SelectNode(String),
    SelectRow(String),
    RunAction(String),
    ConfirmToken(String),
    ConfirmSubmit,
    CancelConfirm,
    Refresh,
    ToggleTree,
    ToggleActions,
    GoBack,
    GoForward,
    GoUp,
    AddRemove,
    ToggleSnap(String),
    ApplyAddRemove,
    CancelAddRemove,
    SaveConsole,
    About,
    Tick,
    ToggleMenu(MenuId),
    CloseMenus,
    ToggleExpand(String),
    DismissOverlay,
    OpenContext(ContextTarget),
    ShowProperties,
    CloseProperties,
    DragStart(DragPane),
    DragMove(f32),
    DragEnd,
    OpenConsole,
    OpenPathInput(String),
    OpenPickPath(String),
    OpenConfirm,
    CancelOpen,
    NewConsole,
    SortColumn(usize),
}

struct Mmc {
    palette: Palette,
    spec: ConsoleSpec,
    root: TreeNode,
    selected: String,
    selection: Option<String>,
    results: ResultsView,
    actions: Vec<ActionItem>,
    status: String,
    history: Vec<String>,
    future: Vec<String>,
    show_tree: bool,
    show_actions: bool,
    dirty: bool,
    confirm: Option<ConfirmState>,
    add_remove: bool,
    pending_snapins: Vec<String>,
    jobs: JobBoard,
    open_menu: Option<MenuId>,
    collapsed: HashSet<String>,
    context: Option<ContextTarget>,
    properties: bool,
    tree_w: f32,
    actions_w: f32,
    dragging: Option<DragPane>,
    drag_origin_x: f32,
    drag_origin_w: f32,
    open_dialog: bool,
    open_path: String,
    sort_col: Option<usize>,
    sort_asc: bool,
}

struct ConfirmState {
    action_id: String,
    prompt: String,
    token: String,
    expect: String,
}

impl Mmc {
    fn new(spec: ConsoleSpec) -> (Self, Task<Message>) {
        let palette = Palette::load();
        let show_tree = spec.tree;
        let show_actions = spec.actions;
        let root = build_root(&spec);
        let selected = root.id.clone();
        let mut app = Self {
            palette,
            spec,
            root,
            selected: selected.clone(),
            selection: None,
            results: ResultsView::default(),
            actions: Vec::new(),
            status: String::new(),
            history: Vec::new(),
            future: Vec::new(),
            show_tree,
            show_actions,
            dirty: false,
            confirm: None,
            add_remove: false,
            pending_snapins: Vec::new(),
            jobs: JobBoard::default(),
            open_menu: None,
            collapsed: HashSet::new(),
            context: None,
            properties: false,
            tree_w: chrome::TREE_W,
            actions_w: chrome::ACTIONS_W,
            dragging: None,
            drag_origin_x: 0.0,
            drag_origin_w: 0.0,
            open_dialog: false,
            open_path: String::new(),
            sort_col: None,
            sort_asc: true,
        };
        app.load_node(&selected, false);
        (app, Task::none())
    }

    fn apply_console(&mut self, spec: ConsoleSpec) {
        self.spec = spec;
        self.show_tree = self.spec.tree;
        self.show_actions = self.spec.actions;
        self.root = build_root(&self.spec);
        self.history.clear();
        self.future.clear();
        self.collapsed.clear();
        self.selection = None;
        self.sort_col = None;
        self.dirty = false;
        let id = self.root.id.clone();
        self.load_node(&id, false);
    }

    fn window_title(&self) -> String {
        let node = self
            .root
            .find(&self.selected)
            .map(|n| n.title.as_str())
            .unwrap_or("Console Root");
        format!("{} - [{}]", self.spec.name, node)
    }

    fn theme(&self) -> Theme {
        mde_theme::iced_theme(&self.palette)
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::Tick),
            iced::keyboard::on_key_press(|key, modifiers| match key.as_ref() {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::F5) => {
                    Some(Message::Refresh)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                    Some(Message::DismissOverlay)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                    if modifiers.alt() =>
                {
                    Some(Message::ShowProperties)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft)
                    if modifiers.alt() =>
                {
                    Some(Message::GoBack)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight)
                    if modifiers.alt() =>
                {
                    Some(Message::GoForward)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp)
                    if modifiers.alt() =>
                {
                    Some(Message::GoUp)
                }
                iced::keyboard::Key::Character("s") if modifiers.control() => {
                    Some(Message::SaveConsole)
                }
                iced::keyboard::Key::Character("m") if modifiers.control() => {
                    Some(Message::AddRemove)
                }
                iced::keyboard::Key::Character("o") if modifiers.control() => {
                    Some(Message::OpenConsole)
                }
                iced::keyboard::Key::Character("n") if modifiers.control() => {
                    Some(Message::NewConsole)
                }
                _ => None,
            }),
            iced::event::listen_with(|event, _status, _id| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::DragMove(position.x))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(_)) => {
                    Some(Message::DragEnd)
                }
                _ => None,
            }),
        ])
    }

    fn load_node(&mut self, id: &str, push_hist: bool) {
        if push_hist && self.selected != id {
            self.history.push(self.selected.clone());
            self.future.clear();
        }
        self.selected = id.to_string();
        self.selection = None;
        self.open_menu = None;
        self.sort_col = None;
        let snap_key = self
            .root
            .find(id)
            .map(|n| n.snapin.clone())
            .unwrap_or_else(|| id.to_string());
        if let Some(snap) = snapin::get(&snap_key) {
            self.results = snap.results(id);
            self.actions = snap.actions(id, None);
            self.status = self.results.status.clone();
        } else if id == "jobs" {
            self.results = ResultsView {
                detail: self
                    .jobs
                    .jobs
                    .iter()
                    .map(|j| {
                        format!(
                            "#{} {} — {}{}",
                            j.id,
                            j.title,
                            j.message,
                            if j.failed {
                                " [FAILED]"
                            } else if j.done {
                                " [done]"
                            } else {
                                " …"
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                status: format!("{} jobs", self.jobs.jobs.len()),
                ..Default::default()
            };
            self.actions = vec![ActionItem::new("refresh", "Refresh")];
        } else {
            self.results = ResultsView {
                detail: format!("Unknown node {id}"),
                ..Default::default()
            };
            self.actions.clear();
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectNode(id) => self.load_node(&id, true),
            Message::SelectRow(id) => {
                self.selection = Some(id.clone());
                let snap_key = self
                    .root
                    .find(&self.selected)
                    .map(|n| n.snapin.clone())
                    .unwrap_or_default();
                if let Some(snap) = snapin::get(&snap_key) {
                    self.actions = snap.actions(&self.selected, Some(&id));
                }
            }
            Message::RunAction(action_id) => {
                self.open_menu = None;
                self.context = None;
                if let Some(act) = self.actions.iter().find(|a| a.id == action_id) {
                    if act.destructive {
                        let expect = self
                            .selection
                            .clone()
                            .unwrap_or_else(|| "CONFIRM".into());
                        self.confirm = Some(ConfirmState {
                            action_id: action_id.clone(),
                            prompt: format!(
                                "Confirm destructive action “{}”. Type “{}” to proceed.",
                                act.label, expect
                            ),
                            token: String::new(),
                            expect,
                        });
                        return Task::none();
                    }
                }
                self.exec_action(&action_id, None);
            }
            Message::ConfirmToken(s) => {
                if let Some(c) = &mut self.confirm {
                    c.token = s;
                }
            }
            Message::ConfirmSubmit => {
                if let Some(c) = self.confirm.take() {
                    if c.token == c.expect {
                        self.exec_action(&c.action_id, Some(&c.token));
                    } else {
                        self.status = "Confirmation text did not match.".into();
                    }
                }
            }
            Message::CancelConfirm => self.confirm = None,
            Message::Refresh => {
                let id = self.selected.clone();
                self.load_node(&id, false);
            }
            Message::ToggleTree => {
                self.show_tree = !self.show_tree;
                self.dirty = true;
                self.open_menu = None;
            }
            Message::ToggleActions => {
                self.show_actions = !self.show_actions;
                self.dirty = true;
                self.open_menu = None;
            }
            Message::GoBack => {
                if let Some(prev) = self.history.pop() {
                    self.future.push(self.selected.clone());
                    self.load_node(&prev, false);
                }
            }
            Message::GoForward => {
                if let Some(next) = self.future.pop() {
                    self.history.push(self.selected.clone());
                    self.load_node(&next, false);
                }
            }
            Message::GoUp => {
                if let Some(p) = self.root.parent_of(&self.selected) {
                    let id = p.id.clone();
                    self.load_node(&id, true);
                }
            }
            Message::AddRemove => {
                self.pending_snapins = self.spec.snapins.clone();
                self.add_remove = true;
                self.open_menu = None;
            }
            Message::ToggleSnap(key) => {
                if let Some(i) = self.pending_snapins.iter().position(|s| s == &key) {
                    self.pending_snapins.remove(i);
                } else {
                    self.pending_snapins.push(key);
                }
            }
            Message::ApplyAddRemove => {
                self.spec.snapins = self.pending_snapins.clone();
                self.root = build_root(&self.spec);
                self.add_remove = false;
                self.dirty = true;
                let id = self.root.id.clone();
                self.load_node(&id, false);
                self.status = "Snap-ins updated".into();
            }
            Message::CancelAddRemove => self.add_remove = false,
            Message::SaveConsole => {
                self.open_menu = None;
                let dir = console::default_save_dir();
                let path = dir.join(format!("{}.msc", sanitize(&self.spec.name)));
                match console::save(&path, &self.spec) {
                    Ok(()) => {
                        self.spec.path = Some(path.display().to_string());
                        self.dirty = false;
                        self.status = format!("Saved {}", path.display());
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::About => {
                self.open_menu = None;
                self.status = format!(
                    "{} Management Console {} — primary admin UI",
                    mde_theme::PRODUCT_SHORT,
                    mde_theme::VERSION
                );
            }
            Message::Tick => {
                if self.selected.starts_with("perfmon") || self.selected == "jobs" {
                    let id = self.selected.clone();
                    self.load_node(&id, false);
                }
            }
            Message::ToggleMenu(id) => {
                self.open_menu = if self.open_menu == Some(id) {
                    None
                } else {
                    Some(id)
                };
            }
            Message::CloseMenus => {
                self.open_menu = None;
                self.context = None;
            }
            Message::ToggleExpand(id) => {
                if !self.collapsed.remove(&id) {
                    self.collapsed.insert(id);
                }
            }
            Message::DismissOverlay => {
                if self.confirm.is_some() {
                    self.confirm = None;
                } else if self.add_remove {
                    self.add_remove = false;
                } else if self.open_dialog {
                    self.open_dialog = false;
                } else if self.properties {
                    self.properties = false;
                } else if self.context.is_some() {
                    self.context = None;
                } else {
                    self.open_menu = None;
                }
            }
            Message::OpenConsole => {
                self.open_menu = None;
                self.open_dialog = true;
                self.open_path.clear();
            }
            Message::OpenPathInput(s) => self.open_path = s,
            Message::OpenPickPath(path) => self.open_path = path,
            Message::OpenConfirm => {
                let path = std::path::PathBuf::from(self.open_path.trim());
                match console::load(&path) {
                    Ok(spec) => {
                        self.open_dialog = false;
                        self.apply_console(spec);
                        self.status = format!("Opened {}", path.display());
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::CancelOpen => self.open_dialog = false,
            Message::NewConsole => {
                self.open_menu = None;
                self.apply_console(ConsoleSpec {
                    name: "Console1".into(),
                    snapins: Vec::new(),
                    tree: true,
                    actions: true,
                    builtin: None,
                    path: None,
                    console: None,
                });
                self.dirty = true;
                self.status = "New empty console".into();
            }
            Message::SortColumn(i) => {
                if self.sort_col == Some(i) {
                    self.sort_asc = !self.sort_asc;
                } else {
                    self.sort_col = Some(i);
                    self.sort_asc = true;
                }
            }
            Message::OpenContext(target) => {
                self.open_menu = None;
                match &target {
                    ContextTarget::Tree(id) => self.load_node(id, true),
                    ContextTarget::Row(id) => {
                        self.selection = Some(id.clone());
                        let snap_key = self
                            .root
                            .find(&self.selected)
                            .map(|n| n.snapin.clone())
                            .unwrap_or_default();
                        if let Some(snap) = snapin::get(&snap_key) {
                            self.actions = snap.actions(&self.selected, Some(id));
                        }
                    }
                }
                self.context = Some(target);
            }
            Message::ShowProperties => {
                self.context = None;
                self.open_menu = None;
                self.properties = true;
            }
            Message::CloseProperties => self.properties = false,
            Message::DragStart(pane) => {
                self.dragging = Some(pane);
                self.drag_origin_w = match pane {
                    DragPane::Tree => self.tree_w,
                    DragPane::Actions => self.actions_w,
                };
                // Origin X filled on first DragMove relative delta via storing current width only;
                // DragMove uses absolute delta from press by capturing first x.
                self.drag_origin_x = f32::NAN;
            }
            Message::DragMove(x) => {
                if let Some(pane) = self.dragging {
                    if self.drag_origin_x.is_nan() {
                        self.drag_origin_x = x;
                        return Task::none();
                    }
                    let dx = x - self.drag_origin_x;
                    match pane {
                        DragPane::Tree => {
                            self.tree_w = (self.drag_origin_w + dx).clamp(180.0, 480.0);
                        }
                        DragPane::Actions => {
                            // Dragging the left edge of the actions pane: moving right shrinks it.
                            self.actions_w = (self.drag_origin_w - dx).clamp(180.0, 420.0);
                        }
                    }
                }
            }
            Message::DragEnd => {
                self.dragging = None;
                self.drag_origin_x = 0.0;
            }
        }
        Task::none()
    }

    fn exec_action(&mut self, action_id: &str, token: Option<&str>) {
        let snap_key = self
            .root
            .find(&self.selected)
            .map(|n| n.snapin.clone())
            .unwrap_or_default();
        let Some(snap) = snapin::get(&snap_key) else {
            self.status = "No snap-in".into();
            return;
        };
        let job = self.jobs.start(format!("{snap_key}:{action_id}"));
        let sel = self.selection.clone();
        match snap.run_action(&self.selected, action_id, sel.as_deref(), token) {
            Ok(msg) => {
                self.jobs.finish(job, true, &msg);
                self.status = msg;
                let id = self.selected.clone();
                self.load_node(&id, false);
            }
            Err(e) => {
                self.jobs.finish(job, false, &e);
                self.status = e;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut main = self.view_shell();
        if let Some(menu) = self.open_menu {
            // Dismiss layer under the dropdown so clicks outside close the menu.
            main = stack![
                main,
                mouse_area(container(Space::new(Fill, Fill)).width(Fill).height(Fill))
                    .on_press(Message::CloseMenus),
                container(self.menu_dropdown(menu))
                    .width(Fill)
                    .height(Fill)
                    .align_left(Fill)
                    .align_top(Fill)
                    .padding(chrome::pad4(28, 8, 0, 8)),
            ]
            .width(Fill)
            .height(Fill)
            .into();
        }
        if self.context.is_some() {
            main = overlay_corner(p, main, self.view_context_menu());
        }
        if self.add_remove {
            return overlay(p, main, self.view_add_remove_card());
        }
        if self.open_dialog {
            return overlay(p, main, self.view_open_card());
        }
        if let Some(c) = &self.confirm {
            return overlay(p, main, self.view_confirm_card(c));
        }
        if self.properties {
            return overlay(p, main, self.view_properties_card());
        }
        main
    }

    fn view_shell(&self) -> Element<'_, Message> {
        let p = &self.palette;

        let menubar = self.view_menubar_bar();
        let toolbar = self.view_toolbar();

        let mut body = row![].spacing(0).padding(chrome::pad());

        if self.show_tree {
            body = body.push(
                container(self.view_tree())
                    .width(self.tree_w)
                    .height(Fill)
                    .style(chrome::pane_style(p)),
            );
            body = body.push(pane_splitter(p, DragPane::Tree));
        }

        body = body.push(
            container(self.view_results())
                .width(Fill)
                .height(Fill)
                .style(chrome::pane_style(p)),
        );

        if self.show_actions {
            body = body.push(pane_splitter(p, DragPane::Actions));
            body = body.push(
                container(self.view_actions())
                    .width(self.actions_w)
                    .height(Fill)
                    .style(chrome::actions_pane_style(p)),
            );
        }

        let status = self.view_statusbar();

        let content = column![
            menubar,
            chrome::chrome_rule(p),
            toolbar,
            chrome::chrome_rule(p),
            body,
            chrome::chrome_rule(p),
            status
        ]
        .width(Fill)
        .height(Fill);

        container(content)
            .width(Fill)
            .height(Fill)
            .style(chrome::canvas_style(p))
            .into()
    }

    fn view_menubar_bar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        row![
            chrome::menu_label_btn(
                p,
                "File",
                self.open_menu == Some(MenuId::File),
                Message::ToggleMenu(MenuId::File)
            ),
            chrome::menu_label_btn(
                p,
                "Action",
                self.open_menu == Some(MenuId::Action),
                Message::ToggleMenu(MenuId::Action)
            ),
            chrome::menu_label_btn(
                p,
                "View",
                self.open_menu == Some(MenuId::View),
                Message::ToggleMenu(MenuId::View)
            ),
            chrome::menu_label_btn(
                p,
                "Help",
                self.open_menu == Some(MenuId::Help),
                Message::ToggleMenu(MenuId::Help)
            ),
        ]
        .spacing(2)
        .padding(chrome::pad4(2, 4, 0, 4))
        .into()
    }

    fn menu_dropdown(&self, menu: MenuId) -> Element<'_, Message> {
        let p = &self.palette;
        let mut items: Vec<Element<'_, Message>> = Vec::new();
        match menu {
            MenuId::File => {
                items.push(menu_row(p, "New", Message::NewConsole));
                items.push(menu_row(p, "Open…", Message::OpenConsole));
                items.push(menu_row(p, "Add/Remove Snap-in…", Message::AddRemove));
                items.push(menu_row(p, "Save", Message::SaveConsole));
            }
            MenuId::Action => {
                items.push(menu_row(p, "Properties", Message::ShowProperties));
                items.push(menu_row(p, "Refresh", Message::Refresh));
                if !self.actions.is_empty() {
                    for a in &self.actions {
                        if a.enabled {
                            items.push(menu_row(
                                p,
                                a.label.as_str(),
                                Message::RunAction(a.id.clone()),
                            ));
                        }
                    }
                }
            }
            MenuId::View => {
                items.push(menu_row(
                    p,
                    if self.show_tree {
                        "Hide Console Tree"
                    } else {
                        "Show Console Tree"
                    },
                    Message::ToggleTree,
                ));
                items.push(menu_row(
                    p,
                    if self.show_actions {
                        "Hide Action Pane"
                    } else {
                        "Show Action Pane"
                    },
                    Message::ToggleActions,
                ));
                items.push(menu_row(p, "Refresh", Message::Refresh));
            }
            MenuId::Help => {
                items.push(menu_row(p, "About Management Console", Message::About));
            }
        }
        let mut col = column![].spacing(2).padding(8);
        for i in items {
            col = col.push(i);
        }
        container(col)
            .width(Length::Fixed(280.0))
            .style(chrome::dialog_card_style(p))
            .into()
    }

    fn view_toolbar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        row![
            chrome::tool_btn(p, ToolKind::Back, Message::GoBack),
            chrome::tool_btn(p, ToolKind::Forward, Message::GoForward),
            chrome::tool_btn(p, ToolKind::Up, Message::GoUp),
            chrome::toolbar_sep(p),
            chrome::tool_btn(p, ToolKind::Tree, Message::ToggleTree),
            chrome::tool_btn(p, ToolKind::Refresh, Message::Refresh),
            chrome::tool_btn(p, ToolKind::Actions, Message::ToggleActions),
            chrome::toolbar_sep(p),
            chrome::tool_btn(p, ToolKind::Properties, Message::ShowProperties),
            chrome::tool_btn(p, ToolKind::Help, Message::About),
        ]
        .spacing(2)
        .padding(chrome::pad4(2, 6, 4, 6))
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_tree(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let mut flat = Vec::new();
        flatten_visible(&self.root, 0, &self.collapsed, &mut flat);
        flat.push((
            0,
            TreeNode::new("jobs", "Jobs", "view-refresh", "jobs")
                .with_desc("Background MMC operations"),
            false,
        ));

        let mut col = column![
            text("Console Root")
                .size(12)
                .color(chrome::muted(p))
                .font(chrome::body_font())
        ]
        .spacing(2)
        .padding(6);

        for (depth, node, has_kids) in flat {
            let selected = node.id == self.selected;
            let collapsed = self.collapsed.contains(&node.id);
            let chevron = if has_kids {
                let glyph = if collapsed { "▸" } else { "▾" };
                button(text(glyph).size(12).color(chrome::muted(p)))
                    .on_press(Message::ToggleExpand(node.id.clone()))
                    .style(chrome::quiet_btn_style(p))
                    .padding(Padding::from([2, 4]))
            } else {
                button(text(" ").size(12))
                    .style(chrome::quiet_btn_style(p))
                    .padding(Padding::from([2, 4]))
            };

            let face = row![
                Space::with_width(Length::Fixed((depth as f32) * 14.0)),
                chevron,
                chrome::icon_for(p, &node.icon, chrome::ICON_TREE),
                text(node.title.clone())
                    .size(13)
                    .color(chrome::fg(p))
                    .font(chrome::body_font()),
            ]
            .spacing(6)
            .padding(Padding::from([4, 6]))
            .align_y(iced::Alignment::Center);

            let id = node.id.clone();
            let row_btn = button(face)
                .on_press(Message::SelectNode(id.clone()))
                .style(chrome::select_row_style(p, selected))
                .padding(0)
                .width(Fill);
            col = col.push(
                mouse_area(row_btn)
                    .on_right_press(Message::OpenContext(ContextTarget::Tree(id))),
            );
        }
        scrollable(col).height(Fill).into()
    }

    fn view_results(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let node = self.root.find(&self.selected);
        let title = node.map(|n| n.title.as_str()).unwrap_or("Results");
        let desc = node.map(|n| n.description.as_str()).unwrap_or("");
        let icon = node.map(|n| n.icon.as_str()).unwrap_or("folder");

        // Banner
        let mut title_col = column![text(title.to_string())
            .size(18)
            .color(chrome::fg(p))
            .font(chrome::body_font())]
        .spacing(2);
        if !desc.is_empty() {
            title_col = title_col.push(
                text(desc.to_string())
                    .size(12)
                    .color(chrome::muted(p)),
            );
        }
        let banner = row![chrome::icon_for(p, icon, 28), title_col,]
            .spacing(12)
            .padding(chrome::pad4(16, 16, 8, 16))
            .align_y(iced::Alignment::Center);

        let mut col = column![banner].spacing(6);

        // Summary chips — quick facts above tables/detail
        if !self.results.summaries.is_empty() {
            let mut chips = row![].spacing(8);
            for s in &self.results.summaries {
                chips = chips.push(chrome::summary_chip(
                    p,
                    s.label.clone(),
                    s.value.clone(),
                    s.tone,
                ));
            }
            col = col.push(chips.padding(chrome::pad4(0, 16, 4, 16)));
        }

        // Gauges / charts for live metrics
        if !self.results.gauges.is_empty() {
            let mut gauges = column![chrome::section_label(p, "Live metrics")]
                .spacing(10)
                .padding(chrome::pad4(4, 16, 8, 16));
            for g in &self.results.gauges {
                gauges = gauges.push(chrome::gauge_row(
                    p,
                    g.label.clone(),
                    g.value,
                    g.display.clone(),
                    g.tone,
                ));
            }
            col = col.push(
                container(gauges)
                    .width(Fill)
                    .style(chrome::actions_pane_style(p))
                    .padding(chrome::pad4(0, 0, 0, 0)),
            );
        }

        // Structured properties (preferred over monospace dumps)
        if !self.results.properties.is_empty() {
            let pairs: Vec<(String, String)> = self
                .results
                .properties
                .iter()
                .map(|pr| (pr.key.clone(), pr.value.clone()))
                .collect();
            col = col.push(
                container(chrome::property_block(p, &pairs))
                    .padding(chrome::pad4(0, 16, 8, 16))
                    .width(Fill),
            );
        }

        let has_table = !self.results.columns.is_empty() || !self.results.rows.is_empty();
        let has_body = has_table
            || !self.results.detail.is_empty()
            || !self.results.properties.is_empty()
            || !self.results.gauges.is_empty()
            || !self.results.summaries.is_empty();

        if !has_body {
            col = col.push(
                container(
                    column![
                        chrome::icon_for(p, icon, 48),
                        text("No items to show in this view.")
                            .size(14)
                            .color(chrome::muted(p)),
                        text("Select a node in the console tree, or use Action › Refresh.")
                            .size(12)
                            .color(chrome::muted(p)),
                    ]
                    .spacing(10)
                    .padding(40)
                    .align_x(iced::Alignment::Center),
                )
                .width(Fill)
                .center_x(Fill),
            );
            return col.into();
        }

        // No table: structured panels above are enough, optional legacy text below
        if !has_table {
            if !self.results.detail.is_empty() {
                col = col.push(
                    scrollable(
                        container(
                            column![
                                chrome::section_label(p, "Output"),
                                text(self.results.detail.clone())
                                    .size(12)
                                    .font(chrome::mono_font())
                                    .color(chrome::fg(p)),
                            ]
                            .spacing(8)
                            .padding(chrome::pad4(0, 16, 16, 16)),
                        ),
                    )
                    .height(Fill),
                );
            }
            return col.into();
        }

        if !self.results.columns.is_empty() {
            let weights = &self.results.column_weights;
            let mut header = row![Space::with_width(Length::Fixed(28.0))]
                .spacing(8)
                .padding(chrome::pad4(4, 12, 4, 12));
            for (i, c) in self.results.columns.iter().enumerate() {
                let marker = match self.sort_col {
                    Some(s) if s == i && self.sort_asc => " ▲",
                    Some(s) if s == i => " ▼",
                    _ => "",
                };
                let w = chrome::col_length(weights, i);
                header = header.push(
                    button(
                        text(format!("{c}{marker}"))
                            .size(11)
                            .color(chrome::muted(p))
                            .width(Fill),
                    )
                    .on_press(Message::SortColumn(i))
                    .style(chrome::quiet_btn_style(p))
                    .padding(Padding::from([2, 4]))
                    .width(w),
                );
            }
            col = col.push(header);
            col = col.push(chrome::chrome_rule(p));
        }

        let mut rows: Vec<&crate::model::ResultRow> = self.results.rows.iter().collect();
        if let Some(ci) = self.sort_col {
            rows.sort_by(|a, b| {
                let ca = a.cells.get(ci).map(|s| s.to_lowercase()).unwrap_or_default();
                let cb = b.cells.get(ci).map(|s| s.to_lowercase()).unwrap_or_default();
                let ord = ca.cmp(&cb);
                if self.sort_asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }

        let weights = self.results.column_weights.clone();
        let columns = self.results.columns.clone();
        let mut list = column![].spacing(1).padding(chrome::pad4(0, 8, 8, 8));
        for row_data in rows {
            let selected = self.selection.as_deref() == Some(row_data.id.as_str());
            let mut face = row![chrome::icon_for(p, &row_data.icon, chrome::ICON_ROW)]
                .spacing(8)
                .padding(chrome::pad4(6, 8, 6, 8))
                .align_y(iced::Alignment::Center);
            if columns.is_empty() {
                face = face.push(
                    text(row_data.cells.join("  "))
                        .size(13)
                        .color(chrome::fg(p))
                        .font(chrome::body_font())
                        .width(Fill),
                );
            } else {
                for (i, col_name) in columns.iter().enumerate() {
                    let cell = row_data.cells.get(i).cloned().unwrap_or_default();
                    let color = chrome::cell_color(p, col_name, &cell);
                    face = face.push(
                        text(cell)
                            .size(13)
                            .color(color)
                            .font(chrome::body_font())
                            .width(chrome::col_length(&weights, i)),
                    );
                }
            }
            let id = row_data.id.clone();
            let row_btn = button(face)
                .on_press(Message::SelectRow(id.clone()))
                .style(chrome::select_row_style(p, selected))
                .padding(0)
                .width(Fill);
            list = list.push(
                mouse_area(row_btn)
                    .on_right_press(Message::OpenContext(ContextTarget::Row(id))),
            );
        }
        if !self.results.detail.is_empty() {
            list = list.push(Space::with_height(8));
            list = list.push(
                container(
                    column![
                        chrome::section_label(p, "Preview"),
                        text(self.results.detail.clone())
                            .size(12)
                            .color(chrome::fg(p))
                            .font(chrome::body_font()),
                    ]
                    .spacing(4)
                    .padding(12),
                )
                .width(Fill)
                .style(chrome::actions_pane_style(p)),
            );
        }
        col = col.push(scrollable(list).height(Fill));
        col.into()
    }

    fn view_actions(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let node_title = self
            .root
            .find(&self.selected)
            .map(|n| n.title.clone())
            .unwrap_or_default();

        let head = container(
            column![
                text("Actions")
                    .size(14)
                    .color(chrome::fg(p))
                    .font(chrome::body_font()),
                text(node_title).size(12).color(chrome::muted(p)),
            ]
            .spacing(2)
            .padding(chrome::pad4(8, 10, 8, 10)),
        )
        .width(Fill)
        .style(move |_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color {
                a: 0.045,
                ..chrome::fg(p)
            })),
            ..Default::default()
        });

        let mut col = column![head].spacing(2).padding(4);
        for a in &self.actions {
            let icon = chrome::action_icon(&a.id);
            if a.enabled {
                let label_color = if a.destructive {
                    chrome::parse(&p.danger).unwrap_or(chrome::fg(p))
                } else {
                    chrome::fg(p)
                };
                let face = row![
                    chrome::icon_for(p, icon, 16),
                    text(a.label.clone())
                        .size(13)
                        .color(label_color)
                        .font(chrome::body_font()),
                ]
                .spacing(10)
                .padding(chrome::pad4(6, 10, 6, 10))
                .align_y(iced::Alignment::Center);
                col = col.push(
                    button(face)
                        .on_press(Message::RunAction(a.id.clone()))
                        .style(chrome::quiet_btn_style(p))
                        .padding(0)
                        .width(Fill),
                );
            } else {
                col = col.push(
                    container(
                        text(format!("{} (unavailable)", a.label))
                            .size(12)
                            .color(chrome::muted(p)),
                    )
                    .padding(chrome::pad4(6, 10, 6, 10)),
                );
            }
        }
        scrollable(col).height(Fill).into()
    }

    fn view_statusbar(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let node = self
            .root
            .find(&self.selected)
            .map(|n| n.title.clone())
            .unwrap_or_else(|| "Console Root".into());
        let sel = self
            .selection
            .as_ref()
            .map(|s| format!("  |  Selected: {s}"))
            .unwrap_or_default();
        let dirty = if self.dirty { "  • unsaved" } else { "" };
        let line = format!(
            "{node}{sel}  |  {}{dirty}",
            if self.status.is_empty() {
                "Ready"
            } else {
                &self.status
            }
        );
        container(
            text(line)
                .size(11)
                .color(chrome::muted(p))
                .font(chrome::body_font()),
        )
        .padding(chrome::pad4(4, 12, 6, 12))
        .width(Fill)
        .into()
    }

    fn view_confirm_card(&self, c: &ConfirmState) -> Element<'_, Message> {
        let p = &self.palette;
        container(
            column![
                text("Confirm")
                    .size(20)
                    .color(chrome::fg(p))
                    .font(chrome::body_font()),
                text(c.prompt.clone())
                    .size(14)
                    .color(chrome::fg(p)),
                text_input("Type confirmation", &c.token).on_input(Message::ConfirmToken),
                row![
                    button(text("Cancel").size(13))
                        .on_press(Message::CancelConfirm)
                        .style(chrome::quiet_btn_style(p))
                        .padding(chrome::pad4(8, 16, 8, 16)),
                    button(text("OK").size(13))
                        .on_press(Message::ConfirmSubmit)
                        .style(iced::widget::button::primary)
                        .padding(chrome::pad4(8, 16, 8, 16)),
                ]
                .spacing(8)
            ]
            .spacing(12)
            .padding(24)
            .width(480),
        )
        .style(chrome::dialog_card_style(p))
        .into()
    }

    fn view_open_card(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let saved = console::list_saved();
        let mut list = column![
            text("Recent / saved consoles")
                .size(13)
                .color(chrome::fg(p))
                .font(chrome::body_font())
        ]
        .spacing(2)
        .padding(8);
        if saved.is_empty() {
            list = list.push(
                text(format!(
                    "No .msc files in {}",
                    console::default_save_dir().display()
                ))
                .size(12)
                .color(chrome::muted(p)),
            );
        } else {
            for path in saved.into_iter().take(24) {
                let label = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("console.msc")
                    .to_string();
                let path_s = path.display().to_string();
                list = list.push(
                    button(
                        row![
                            chrome::icon_for(p, "document-open", 16),
                            text(label)
                                .size(13)
                                .color(chrome::fg(p))
                                .font(chrome::body_font()),
                        ]
                        .spacing(8)
                        .padding(chrome::pad4(6, 8, 6, 8))
                        .align_y(iced::Alignment::Center),
                    )
                    .on_press(Message::OpenPickPath(path_s))
                    .style(chrome::quiet_btn_style(p))
                    .padding(0)
                    .width(Fill),
                );
            }
        }

        let col = column![
            text("Open Console")
                .size(20)
                .color(chrome::fg(p))
                .font(chrome::body_font()),
            text("Choose a saved console or type a path to a .msc file.")
                .size(13)
                .color(chrome::muted(p)),
            container(scrollable(list).height(Length::Fixed(220.0)))
                .style(chrome::pane_style(p))
                .width(Fill),
            text_input("Path to .msc", &self.open_path).on_input(Message::OpenPathInput),
            row![
                button(text("Cancel").size(13))
                    .on_press(Message::CancelOpen)
                    .style(chrome::quiet_btn_style(p))
                    .padding(chrome::pad4(8, 16, 8, 16)),
                button(text("Open").size(13))
                    .on_press(Message::OpenConfirm)
                    .style(iced::widget::button::primary)
                    .padding(chrome::pad4(8, 16, 8, 16)),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(24)
        .width(560);

        container(col).style(chrome::dialog_card_style(p)).into()
    }

    fn view_context_menu(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let title = match &self.context {
            Some(ContextTarget::Tree(id)) => self
                .root
                .find(id)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| id.clone()),
            Some(ContextTarget::Row(id)) => id.clone(),
            None => "Context".into(),
        };
        let mut col = column![
            text(title)
                .size(12)
                .color(chrome::muted(p))
                .font(chrome::body_font()),
            menu_row(p, "Properties", Message::ShowProperties),
            menu_row(p, "Refresh", Message::Refresh),
        ]
        .spacing(2)
        .padding(8)
        .width(260);
        for a in &self.actions {
            if a.enabled {
                col = col.push(menu_row(
                    p,
                    a.label.as_str(),
                    Message::RunAction(a.id.clone()),
                ));
            }
        }
        container(col).style(chrome::dialog_card_style(p)).into()
    }

    fn view_properties_card(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let node = self.root.find(&self.selected);
        let title = node.map(|n| n.title.as_str()).unwrap_or("Properties");
        let icon = node.map(|n| n.icon.as_str()).unwrap_or("document-properties");
        let desc = node.map(|n| n.description.as_str()).unwrap_or("");

        let mut col = column![
            row![
                chrome::icon_for(p, icon, 32),
                column![
                    text(format!("{title} Properties"))
                        .size(18)
                        .color(chrome::fg(p))
                        .font(chrome::body_font()),
                    text(desc.to_string())
                        .size(12)
                        .color(chrome::muted(p)),
                ]
                .spacing(2),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            Space::with_height(8),
            prop_row(p, "Node", &self.selected),
            prop_row(p, "Console", &self.spec.name),
            prop_row(
                p,
                "Snap-in",
                node.map(|n| n.snapin.as_str()).unwrap_or("-"),
            ),
        ]
        .spacing(6)
        .padding(24)
        .width(520);

        if let Some(sel) = &self.selection {
            col = col.push(Space::with_height(8));
            col = col.push(
                text("Selection")
                    .size(14)
                    .color(chrome::fg(p))
                    .font(chrome::body_font()),
            );
            col = col.push(prop_row(p, "Id", sel));
            if let Some(row) = self.results.rows.iter().find(|r| &r.id == sel) {
                for (i, name) in self.results.columns.iter().enumerate() {
                    let val = row.cells.get(i).map(|s| s.as_str()).unwrap_or("");
                    col = col.push(prop_row(p, name, val));
                }
                if !row.icon.is_empty() {
                    col = col.push(prop_row(p, "Icon", &row.icon));
                }
            }
        }

        if !self.results.detail.is_empty() && self.results.detail.len() < 2000 {
            col = col.push(Space::with_height(8));
            col = col.push(
                text("Detail")
                    .size(14)
                    .color(chrome::fg(p))
                    .font(chrome::body_font()),
            );
            col = col.push(
                text(self.results.detail.clone())
                    .size(12)
                    .font(chrome::mono_font())
                    .color(chrome::fg(p)),
            );
        }

        col = col.push(Space::with_height(12));
        col = col.push(
            button(text("Close").size(13))
                .on_press(Message::CloseProperties)
                .style(chrome::quiet_btn_style(p))
                .padding(chrome::pad4(8, 16, 8, 16)),
        );

        container(scrollable(col).height(Length::Fixed(520.0)))
            .style(chrome::dialog_card_style(p))
            .into()
    }

    fn view_add_remove_card(&self) -> Element<'_, Message> {
        let p = &self.palette;
        let meta = snapin::all_meta();
        let available: Vec<_> = meta
            .iter()
            .filter(|m| !self.pending_snapins.iter().any(|s| s == m.key))
            .collect();
        let selected: Vec<_> = self
            .pending_snapins
            .iter()
            .filter_map(|k| meta.iter().find(|m| m.key == k.as_str()))
            .collect();

        let mut avail_col = column![
            text("Available snap-ins")
                .size(13)
                .color(chrome::fg(p))
                .font(chrome::body_font())
        ]
        .spacing(2)
        .padding(8)
        .width(Fill);
        for m in &available {
            let face = row![
                chrome::icon_for(p, m.icon, 16),
                text(m.name.to_string())
                    .size(13)
                    .color(chrome::fg(p))
                    .font(chrome::body_font()),
            ]
            .spacing(8)
            .padding(chrome::pad4(6, 8, 6, 8))
            .align_y(iced::Alignment::Center);
            avail_col = avail_col.push(
                button(face)
                    .on_press(Message::ToggleSnap(m.key.to_string()))
                    .style(chrome::quiet_btn_style(p))
                    .padding(0)
                    .width(Fill),
            );
        }

        let mut sel_col = column![
            text("Snap-ins in this console")
                .size(13)
                .color(chrome::fg(p))
                .font(chrome::body_font())
        ]
        .spacing(2)
        .padding(8)
        .width(Fill);
        for m in &selected {
            let face = row![
                chrome::icon_for(p, m.icon, 16),
                text(m.name.to_string())
                    .size(13)
                    .color(chrome::fg(p))
                    .font(chrome::body_font()),
            ]
            .spacing(8)
            .padding(chrome::pad4(6, 8, 6, 8))
            .align_y(iced::Alignment::Center);
            sel_col = sel_col.push(
                button(face)
                    .on_press(Message::ToggleSnap(m.key.to_string()))
                    .style(chrome::select_row_style(p, true))
                    .padding(0)
                    .width(Fill),
            );
        }

        let lists = row![
            container(scrollable(avail_col).height(Fill))
                .width(Fill)
                .height(Fill)
                .style(chrome::pane_style(p)),
            column![
                text("Add →")
                    .size(12)
                    .color(chrome::muted(p)),
                text("← Remove")
                    .size(12)
                    .color(chrome::muted(p)),
                text("Click a snap-in\nto move it.")
                    .size(11)
                    .color(chrome::muted(p)),
            ]
            .spacing(8)
            .padding(12)
            .align_x(iced::Alignment::Center),
            container(scrollable(sel_col).height(Fill))
                .width(Fill)
                .height(Fill)
                .style(chrome::pane_style(p)),
        ]
        .spacing(12)
        .height(Length::Fixed(360.0));

        let col = column![
            text("Add or Remove Snap-ins")
                .size(20)
                .color(chrome::fg(p))
                .font(chrome::body_font()),
            text("Separate snap-ins can be added to this console.")
                .size(13)
                .color(chrome::muted(p)),
            lists,
            row![
                button(text("Cancel").size(13))
                    .on_press(Message::CancelAddRemove)
                    .style(chrome::quiet_btn_style(p))
                    .padding(chrome::pad4(8, 16, 8, 16)),
                button(text("OK").size(13))
                    .on_press(Message::ApplyAddRemove)
                    .style(iced::widget::button::primary)
                    .padding(chrome::pad4(8, 16, 8, 16)),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(24)
        .width(720);

        container(col).style(chrome::dialog_card_style(p)).into()
    }
}

fn overlay<'a>(
    palette: &Palette,
    base: Element<'a, Message>,
    card: Element<'a, Message>,
) -> Element<'a, Message> {
    stack![
        base,
        mouse_area(
            container(Space::new(Fill, Fill))
                .width(Fill)
                .height(Fill)
                .style(chrome::dim_style_for(palette.dark)),
        )
        .on_press(Message::DismissOverlay),
        container(card)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

fn overlay_corner<'a>(
    _palette: &Palette,
    base: Element<'a, Message>,
    card: Element<'a, Message>,
) -> Element<'a, Message> {
    stack![
        base,
        mouse_area(container(Space::new(Fill, Fill)).width(Fill).height(Fill))
            .on_press(Message::CloseMenus),
        container(container(card).padding(chrome::pad4(72, 16, 16, 16)))
            .width(Fill)
            .height(Fill)
            .align_left(Fill)
            .align_top(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

fn pane_splitter(palette: &Palette, pane: DragPane) -> Element<'static, Message> {
    let edge = chrome::border(palette);
    let hc = palette.high_contrast;
    mouse_area(
        container(Space::new(6, Fill))
            .width(6)
            .height(Fill)
            .style(move |_theme| iced::widget::container::Style {
                // Quiet grip so paned splits read as MMC chrome, not invisible padding.
                background: Some(iced::Background::Color(iced::Color {
                    a: if hc { 1.0 } else { 0.14 },
                    ..edge
                })),
                ..Default::default()
            }),
    )
    .on_press(Message::DragStart(pane))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

fn prop_row<'a>(palette: &Palette, key: &str, value: &str) -> Element<'a, Message> {
    row![
        text(key.to_string())
            .size(12)
            .color(chrome::muted(palette))
            .width(Length::Fixed(120.0)),
        text(value.to_string())
            .size(12)
            .color(chrome::fg(palette))
            .font(chrome::body_font())
            .width(Fill),
    ]
    .spacing(8)
    .into()
}

fn resolve_window_icon(spec: &ConsoleSpec) -> Option<iced::window::Icon> {
    let key = spec
        .snapins
        .first()
        .map(|s| s.as_str())
        .unwrap_or("compmgmt");
    let name = snapin::all_meta()
        .into_iter()
        .find(|m| m.key == key)
        .map(|m| m.icon)
        .unwrap_or("applications-system");
    let dark_ui = mde_theme::Palette::load().dark;
    let path = mde_theme::icons::for_ui(name, 48, dark_ui)?;
    iced::window::icon::from_file(path).ok()
}

fn menu_row(palette: &Palette, label: &str, msg: Message) -> Element<'static, Message> {
    button(
        text(label.to_string())
            .size(13)
            .color(chrome::fg(palette))
            .font(chrome::body_font())
            .width(Fill),
    )
    .on_press(msg)
    .style(chrome::quiet_btn_style(palette))
    .padding(chrome::pad4(6, 10, 6, 10))
    .width(Fill)
    .into()
}

fn flatten_visible(
    node: &TreeNode,
    depth: usize,
    collapsed: &HashSet<String>,
    out: &mut Vec<(usize, TreeNode, bool)>,
) {
    let has_kids = !node.children.is_empty();
    out.push((depth, node.clone(), has_kids));
    if has_kids && !collapsed.contains(&node.id) {
        for c in &node.children {
            flatten_visible(c, depth + 1, collapsed, out);
        }
    }
}

fn build_root(spec: &ConsoleSpec) -> TreeNode {
    if spec.snapins.len() == 1 {
        if let Some(s) = snapin::get(&spec.snapins[0]) {
            return s.root();
        }
    }
    let mut kids = Vec::new();
    for k in &spec.snapins {
        if let Some(s) = snapin::get(k) {
            kids.push(s.root());
        }
    }
    if kids.is_empty() {
        if let Some(s) = snapin::get("compmgmt") {
            return s.root();
        }
    }
    TreeNode::new("root", "Console Root", "folder", "root").with_children(kids)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
