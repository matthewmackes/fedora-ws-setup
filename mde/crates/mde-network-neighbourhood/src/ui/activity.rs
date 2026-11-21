use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, Orientation, Revealer, ScrolledWindow, TextBuffer,
    TextView,
};

/// Collapsible live log under the command bar for Refresh / discovery.
pub struct ActivityPanel {
    pub revealer: Revealer,
    title: Label,
    buffer: TextBuffer,
}

impl ActivityPanel {
    pub fn new() -> Self {
        let revealer = Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideDown)
            .reveal_child(false)
            .build();

        let box_ = GtkBox::new(Orientation::Vertical, 4);
        box_.set_margin_start(10);
        box_.set_margin_end(10);
        box_.set_margin_top(6);
        box_.set_margin_bottom(6);
        box_.add_css_class("nn-split-pane");

        let header = GtkBox::new(Orientation::Horizontal, 8);
        let title = Label::builder()
            .label("Activity")
            .halign(Align::Start)
            .hexpand(true)
            .build();
        let hide = Button::with_label("Hide");
        hide.add_css_class("flat");
        header.append(&title);
        header.append(&hide);
        box_.append(&header);

        let buffer = TextBuffer::new(None::<&gtk4::TextTagTable>);
        let view = TextView::with_buffer(&buffer);
        view.set_editable(false);
        view.set_monospace(true);
        view.set_wrap_mode(gtk4::WrapMode::WordChar);
        view.add_css_class("nn-log");
        view.set_size_request(-1, 140);
        let scroll = ScrolledWindow::builder()
            .child(&view)
            .min_content_height(120)
            .max_content_height(220)
            .hexpand(true)
            .vexpand(false)
            .build();
        box_.append(&scroll);

        revealer.set_child(Some(&box_));

        hide.connect_clicked({
            let revealer = revealer.clone();
            move |_| revealer.set_reveal_child(false)
        });

        Self {
            revealer,
            title,
            buffer,
        }
    }

    pub fn show(&self, title: &str) {
        self.title.set_text(title);
        self.buffer.set_text("");
        self.revealer.set_reveal_child(true);
    }

    pub fn clear(&self) {
        self.buffer.set_text("");
    }

    pub fn append(&self, line: &str) {
        let mut end = self.buffer.end_iter();
        self.buffer.insert(&mut end, line);
        if !line.ends_with('\n') {
            let mut end = self.buffer.end_iter();
            self.buffer.insert(&mut end, "\n");
        }
        // Keep scrolled to end
        let end = self.buffer.end_iter();
        self.buffer.place_cursor(&end);
    }

    pub fn set_title(&self, title: &str) {
        self.title.set_text(title);
    }
}
