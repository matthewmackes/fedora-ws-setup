//! Inline place-row connect status (phrase + expand log + determinate progress).

use crate::connect::{ConnectProgress, ConnectState, ConnectStep};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, Orientation, ProgressBar, Revealer, RevealerTransitionType,
};
use std::cell::Cell;
use std::rc::Rc;

pub struct InlineConnectStatus {
    pub root: GtkBox,
    phrase: Label,
    path_label: Label,
    progress: ProgressBar,
    expand_btn: Button,
    cancel_btn: Button,
    log_box: GtkBox,
    revealer: Revealer,
    actions: GtkBox,
    pub retry_btn: Button,
    pub edit_btn: Button,
    pub forget_btn: Button,
    pub copy_btn: Button,
    pub logs_btn: Button,
    expanded: Rc<Cell<bool>>,
    last_log: Rc<Cell<String>>,
}

impl InlineConnectStatus {
    pub fn new() -> Self {
        let root = GtkBox::new(Orientation::Vertical, 4);
        root.add_css_class("nn-connect-status");
        root.set_hexpand(true);

        let head = GtkBox::new(Orientation::Horizontal, 6);
        head.set_hexpand(true);

        let phrase = Label::builder()
            .label("")
            .halign(Align::Start)
            .hexpand(true)
            .wrap(true)
            .css_classes(["nn-connect-phrase"])
            .build();

        let expand_btn = Button::from_icon_name("pan-down-symbolic");
        expand_btn.set_tooltip_text(Some("Show connect steps"));
        expand_btn.add_css_class("flat");
        expand_btn.set_visible(false);

        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        cancel_btn.set_visible(false);

        head.append(&phrase);
        head.append(&expand_btn);
        head.append(&cancel_btn);

        let path_label = Label::builder()
            .label("")
            .halign(Align::Start)
            .selectable(true)
            .css_classes(["nn-path"])
            .build();
        path_label.set_visible(false);

        let progress = ProgressBar::new();
        progress.set_hexpand(true);
        progress.set_fraction(0.0);
        progress.set_visible(false);
        progress.add_css_class("nn-connect-progress");

        let log_box = GtkBox::new(Orientation::Vertical, 2);
        log_box.set_margin_start(4);
        let revealer = Revealer::builder()
            .transition_type(RevealerTransitionType::SlideDown)
            .reveal_child(false)
            .child(&log_box)
            .build();

        let actions = GtkBox::new(Orientation::Horizontal, 6);
        actions.set_halign(Align::Start);
        actions.set_visible(false);
        let retry_btn = Button::with_label("Retry");
        retry_btn.add_css_class("nn-accent");
        let edit_btn = Button::with_label("Edit");
        let forget_btn = Button::with_label("Forget");
        forget_btn.add_css_class("destructive-action");
        let copy_btn = Button::with_label("Copy error");
        let logs_btn = Button::with_label("Open logs");
        for b in [&retry_btn, &edit_btn, &forget_btn, &copy_btn, &logs_btn] {
            actions.append(b);
        }

        root.append(&head);
        root.append(&path_label);
        root.append(&progress);
        root.append(&revealer);
        root.append(&actions);

        let expanded = Rc::new(Cell::new(false));
        let last_log = Rc::new(Cell::new(String::new()));

        let revealer2 = revealer.clone();
        let expand_btn2 = expand_btn.clone();
        let expanded2 = expanded.clone();
        expand_btn.connect_clicked(move |_| {
            let next = !expanded2.get();
            expanded2.set(next);
            revealer2.set_reveal_child(next);
            expand_btn2.set_icon_name(if next {
                "pan-up-symbolic"
            } else {
                "pan-down-symbolic"
            });
        });

        Self {
            root,
            phrase,
            path_label,
            progress,
            expand_btn,
            cancel_btn,
            log_box,
            revealer,
            actions,
            retry_btn,
            edit_btn,
            forget_btn,
            copy_btn,
            logs_btn,
            expanded,
            last_log,
        }
    }

    pub fn connect_cancel<F: Fn() + 'static>(&self, f: F) {
        self.cancel_btn.connect_clicked(move |_| f());
    }

    pub fn error_text(&self) -> String {
        let s = self.last_log.take();
        self.last_log.set(s.clone());
        s
    }

    pub fn apply(&self, progress: &ConnectProgress) {
        match progress.state {
            ConnectState::Idle => {
                self.root.set_visible(false);
            }
            ConnectState::Connecting => {
                self.root.set_visible(true);
                self.actions.set_visible(false);
                self.cancel_btn.set_visible(true);
                self.progress.set_visible(true);
                self.progress.set_fraction(progress.fraction.clamp(0.0, 1.0));
                self.phrase.set_text(if progress.phrase.is_empty() {
                    "Connecting…"
                } else {
                    &progress.phrase
                });
                self.path_label.set_visible(false);
                self.fill_steps(&progress.steps, false);
                self.expand_btn.set_visible(!progress.steps.is_empty());
            }
            ConnectState::Connected => {
                self.root.set_visible(true);
                self.actions.set_visible(false);
                self.cancel_btn.set_visible(false);
                self.progress.set_visible(false);
                self.progress.set_fraction(1.0);
                let phrase = if progress.phrase.is_empty() {
                    format!("Connected — {}", progress.mount_point)
                } else {
                    progress.phrase.clone()
                };
                self.phrase.set_text(&phrase);
                self.path_label.set_text(&progress.mount_point);
                self.path_label.set_visible(!progress.mount_point.is_empty());
                self.fill_steps(&progress.steps, false);
                self.expand_btn.set_visible(!progress.steps.is_empty());
            }
            ConnectState::AlreadyMounted => {
                self.root.set_visible(true);
                self.actions.set_visible(false);
                self.cancel_btn.set_visible(false);
                self.progress.set_visible(false);
                self.phrase
                    .set_text(&format!("Already mounted — {}", progress.mount_point));
                self.path_label.set_text(&progress.mount_point);
                self.path_label.set_visible(!progress.mount_point.is_empty());
                // No expand log for already-mounted short path.
                while let Some(c) = self.log_box.first_child() {
                    self.log_box.remove(&c);
                }
                self.revealer.set_reveal_child(false);
                self.expanded.set(false);
                self.expand_btn.set_visible(false);
            }
            ConnectState::Failed | ConnectState::Cancelled => {
                self.root.set_visible(true);
                self.cancel_btn.set_visible(false);
                self.progress.set_visible(false);
                self.phrase.set_text(if progress.phrase.is_empty() {
                    "Failed"
                } else {
                    &progress.phrase
                });
                self.phrase.add_css_class("nn-conflict");
                self.path_label.set_visible(false);
                self.fill_steps(&progress.steps, true);
                self.expand_btn.set_visible(true);
                // Auto-expand on failure so the failing step is visible.
                self.expanded.set(true);
                self.revealer.set_reveal_child(true);
                self.expand_btn.set_icon_name("pan-up-symbolic");
                self.actions.set_visible(true);
                let err = if progress.error.is_empty() {
                    progress
                        .steps
                        .iter()
                        .rev()
                        .find(|s| !s.ok)
                        .map(|s| s.detail.clone())
                        .unwrap_or_else(|| progress.phrase.clone())
                } else {
                    progress.error.clone()
                };
                self.last_log.set(err);
            }
        }
    }

    fn fill_steps(&self, steps: &[ConnectStep], _failed: bool) {
        while let Some(c) = self.log_box.first_child() {
            self.log_box.remove(&c);
        }
        for step in steps {
            let line = GtkBox::new(Orientation::Horizontal, 6);
            let mark = Label::new(Some(if step.ok { "✓" } else { "✗" }));
            mark.add_css_class(if step.ok {
                "nn-badge-ok"
            } else {
                "nn-badge-off"
            });
            let text = Label::builder()
                .label(&format!("{} — {}", step.label, step.detail))
                .halign(Align::Start)
                .hexpand(true)
                .wrap(true)
                .selectable(true)
                .css_classes(["nn-connect-step"])
                .build();
            line.append(&mark);
            line.append(&text);
            self.log_box.append(&line);
        }
    }

    pub fn show_connecting_local(&self, name: &str) {
        let mut p = ConnectProgress::new("", "");
        p.state = ConnectState::Connecting;
        p.phrase = format!("Connecting to {name}…");
        p.fraction = 0.05;
        self.apply(&p);
    }
}
