//! Modal "Connecting" dialog: step checklist, spinner, progress bar, collapsed
//! live log, Cancel. On failure it shows the diagnosed cause and fix with
//! Retry / Edit place / Copy log and a one-click auto-fix where one exists.

use crate::connect::{ConnectProgress, ConnectState};
use crate::diagnose::AutoFix;
use gtk4::prelude::*;
use gtk4::{
    Align, ApplicationWindow, Box as GtkBox, Button, Expander, Label, Orientation, ProgressBar,
    ScrolledWindow, Spinner, Window,
};
use std::cell::RefCell;
use std::process::{Command, Stdio};
use std::rc::Rc;

type Cb = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

pub struct ConnectDialog {
    win: Window,
    title: Label,
    spinner: Spinner,
    progress: ProgressBar,
    steps: GtkBox,
    cause: Label,
    fix: Label,
    log: Label,
    expander: Expander,
    cancel_btn: Button,
    retry_btn: Button,
    edit_btn: Button,
    copy_btn: Button,
    fix_btn: Button,
    close_btn: Button,
    host: String,
    auto_fix: Rc<RefCell<Option<AutoFix>>>,
    raw_log: Rc<RefCell<String>>,
    on_retry: Cb,
    on_edit: Cb,
    on_cancel: Cb,
}

impl ConnectDialog {
    pub fn new(parent: &ApplicationWindow, place_name: &str, host: &str) -> Rc<Self> {
        let win = Window::builder()
            .transient_for(parent)
            .modal(true)
            .title(&format!("Connecting to {place_name}"))
            .default_width(460)
            .resizable(false)
            .build();
        win.add_css_class("nn-connect-dialog");

        let root = GtkBox::new(Orientation::Vertical, 12);
        root.set_margin_top(20);
        root.set_margin_bottom(16);
        root.set_margin_start(20);
        root.set_margin_end(20);

        let head = GtkBox::new(Orientation::Horizontal, 10);
        let spinner = Spinner::new();
        spinner.start();
        let title = Label::builder()
            .label(&format!("Connecting to {place_name}…"))
            .halign(Align::Start)
            .hexpand(true)
            .wrap(true)
            .css_classes(["title-3"])
            .build();
        head.append(&spinner);
        head.append(&title);

        let progress = ProgressBar::new();
        progress.set_fraction(0.02);

        let steps = GtkBox::new(Orientation::Vertical, 4);

        let cause = Label::builder()
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.0)
            .css_classes(["nn-conflict"])
            .visible(false)
            .build();
        let fix = Label::builder()
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.0)
            .visible(false)
            .build();

        let log = Label::builder()
            .halign(Align::Start)
            .xalign(0.0)
            .selectable(true)
            .wrap(true)
            .css_classes(["monospace", "nn-connect-step"])
            .build();
        let scroll = ScrolledWindow::builder()
            .min_content_height(110)
            .max_content_height(220)
            .child(&log)
            .build();
        let expander = Expander::builder().label("Details").child(&scroll).build();
        expander.set_expanded(false);

        let buttons = GtkBox::new(Orientation::Horizontal, 8);
        buttons.set_halign(Align::End);
        let cancel_btn = Button::with_label("Cancel");
        let fix_btn = Button::new();
        fix_btn.add_css_class("suggested-action");
        fix_btn.set_visible(false);
        let retry_btn = Button::with_label("Retry");
        retry_btn.set_visible(false);
        let edit_btn = Button::with_label("Edit place");
        edit_btn.set_visible(false);
        let copy_btn = Button::with_label("Copy log");
        copy_btn.set_visible(false);
        let close_btn = Button::with_label("Close");
        close_btn.set_visible(false);
        for b in [&copy_btn, &edit_btn, &retry_btn, &fix_btn, &cancel_btn, &close_btn] {
            buttons.append(b);
        }

        root.append(&head);
        root.append(&progress);
        root.append(&steps);
        root.append(&cause);
        root.append(&fix);
        root.append(&expander);
        root.append(&buttons);
        win.set_child(Some(&root));

        let d = Rc::new(Self {
            win,
            title,
            spinner,
            progress,
            steps,
            cause,
            fix,
            log,
            expander,
            cancel_btn,
            retry_btn,
            edit_btn,
            copy_btn,
            fix_btn,
            close_btn,
            host: host.to_string(),
            auto_fix: Rc::new(RefCell::new(None)),
            raw_log: Rc::new(RefCell::new(String::new())),
            on_retry: Rc::new(RefCell::new(None)),
            on_edit: Rc::new(RefCell::new(None)),
            on_cancel: Rc::new(RefCell::new(None)),
        });
        d.wire();
        d
    }

    fn wire(self: &Rc<Self>) {
        let call = |cb: &Cb| {
            let cb = cb.borrow().clone();
            if let Some(f) = cb {
                f();
            }
        };
        let me = self.clone();
        self.cancel_btn.connect_clicked(move |_| {
            call(&me.on_cancel);
            me.win.close();
        });
        let me = self.clone();
        self.close_btn.connect_clicked(move |_| me.win.close());
        let me = self.clone();
        self.retry_btn.connect_clicked(move |_| {
            me.win.close();
            call(&me.on_retry);
        });
        let me = self.clone();
        self.edit_btn.connect_clicked(move |_| {
            me.win.close();
            call(&me.on_edit);
        });
        let me = self.clone();
        self.copy_btn.connect_clicked(move |_| {
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&me.raw_log.borrow());
            }
        });
        let me = self.clone();
        self.fix_btn.connect_clicked(move |b| {
            let Some(fix) = me.auto_fix.borrow().clone() else { return };
            match fix {
                AutoFix::Retry => {
                    me.win.close();
                    call(&me.on_retry);
                }
                AutoFix::ReenterPassword => {
                    me.win.close();
                    call(&me.on_edit);
                }
                other => {
                    b.set_sensitive(false);
                    b.set_label("Working…");
                    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
                    let host = me.host.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(run_auto_fix(&other, &host));
                    });
                    let me2 = me.clone();
                    gtk4::glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                        match rx.try_recv() {
                            Ok(Ok(())) => {
                                me2.win.close();
                                call(&me2.on_retry);
                                gtk4::glib::ControlFlow::Break
                            }
                            Ok(Err(e)) => {
                                me2.fix_btn.set_sensitive(true);
                                me2.fix_btn.set_label("Try fix again");
                                me2.fix.set_text(&format!("Fix failed: {e}"));
                                gtk4::glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                gtk4::glib::ControlFlow::Continue
                            }
                            Err(_) => gtk4::glib::ControlFlow::Break,
                        }
                    });
                }
            }
        });
    }

    pub fn set_on_retry<F: Fn() + 'static>(&self, f: F) {
        *self.on_retry.borrow_mut() = Some(Rc::new(f));
    }
    pub fn set_on_edit<F: Fn() + 'static>(&self, f: F) {
        *self.on_edit.borrow_mut() = Some(Rc::new(f));
    }
    pub fn set_on_cancel<F: Fn() + 'static>(&self, f: F) {
        *self.on_cancel.borrow_mut() = Some(Rc::new(f));
    }

    pub fn present(&self) {
        self.win.present();
    }

    pub fn close(&self) {
        self.win.close();
    }

    /// Reflect the latest daemon progress.
    pub fn apply(&self, p: &ConnectProgress) {
        rebuild_steps(&self.steps, p);
        let log: String = p
            .steps
            .iter()
            .map(|s| format!("{} {} — {}", if s.ok { "✓" } else { "✗" }, s.label, s.detail))
            .collect::<Vec<_>>()
            .join("\n");
        match p.state {
            ConnectState::Idle | ConnectState::Connecting => {
                self.progress.set_fraction(p.fraction.clamp(0.0, 1.0));
                if !p.phrase.is_empty() {
                    self.title.set_text(&p.phrase);
                }
                self.log.set_text(&log);
                *self.raw_log.borrow_mut() = log;
            }
            ConnectState::Connected | ConnectState::AlreadyMounted => {
                self.win.close();
            }
            ConnectState::Cancelled => {
                self.win.close();
            }
            ConnectState::Failed => {
                self.spinner.stop();
                self.spinner.set_visible(false);
                self.progress.set_visible(false);
                self.cancel_btn.set_visible(false);
                self.title.set_text("Couldn't connect");
                self.cause.set_text(&p.diagnosis.cause);
                self.cause.set_visible(!p.diagnosis.cause.is_empty());
                self.fix.set_text(&p.diagnosis.fix);
                self.fix.set_visible(!p.diagnosis.fix.is_empty());
                let full = format!("{log}\n\nERROR: {}", p.error);
                self.log.set_text(&full);
                *self.raw_log.borrow_mut() = full;
                self.expander.set_expanded(false);
                *self.auto_fix.borrow_mut() = p.diagnosis.auto_fix.clone();
                match &p.diagnosis.auto_fix {
                    Some(AutoFix::InstallPackage(pkg)) => {
                        self.fix_btn.set_label(&format!("Install {pkg}"));
                        self.fix_btn.set_visible(true);
                    }
                    Some(AutoFix::TrustNewHostKey) => {
                        self.fix_btn.set_label("Trust new key");
                        self.fix_btn.set_visible(true);
                    }
                    Some(AutoFix::ReenterPassword) => {
                        self.fix_btn.set_label("Re-enter password");
                        self.fix_btn.set_visible(true);
                    }
                    _ => self.fix_btn.set_visible(false),
                }
                self.retry_btn.set_visible(true);
                self.edit_btn.set_visible(true);
                self.copy_btn.set_visible(true);
                self.close_btn.set_visible(true);
            }
        }
    }
}

fn rebuild_steps(steps: &GtkBox, p: &ConnectProgress) {
    while let Some(c) = steps.first_child() {
        steps.remove(&c);
    }
    for s in &p.steps {
        let line = GtkBox::new(Orientation::Horizontal, 8);
        let mark = Label::new(Some(if s.ok { "✓" } else { "✗" }));
        mark.add_css_class(if s.ok { "nn-badge-ok" } else { "nn-badge-off" });
        let text = Label::builder().label(&s.label).halign(Align::Start).build();
        line.append(&mark);
        line.append(&text);
        steps.append(&line);
    }
}

/// Run a one-click remedy with administrator rights (polkit).
fn run_auto_fix(fix: &AutoFix, host: &str) -> Result<(), String> {
    let mut cmd = Command::new("pkexec");
    match fix {
        AutoFix::InstallPackage(pkg) => {
            cmd.args(["dnf", "install", "-y", pkg]);
        }
        AutoFix::TrustNewHostKey => {
            // The daemon mounts as root, so clear root's known_hosts entry.
            cmd.args(["ssh-keygen", "-R", host]);
        }
        _ => return Ok(()),
    }
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("pkexec: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr);
        Err(msg.trim().chars().take(200).collect())
    }
}
