//! Cancellable job tracking (W455).

use crate::model::Job;

#[derive(Default)]
pub struct JobBoard {
    next_id: u64,
    pub jobs: Vec<Job>,
}

impl JobBoard {
    pub fn start(&mut self, title: impl Into<String>) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.jobs.push(Job {
            id,
            title: title.into(),
            progress: None,
            cancellable: true,
            done: false,
            failed: false,
            message: "Running…".into(),
        });
        id
    }

    pub fn finish(&mut self, id: u64, ok: bool, message: impl Into<String>) {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
            j.done = true;
            j.failed = !ok;
            j.progress = Some(1.0);
            j.message = message.into();
        }
        notify_done(ok, &self.jobs.iter().find(|j| j.id == id).map(|j| j.title.clone()).unwrap_or_default());
    }

    pub fn active(&self) -> impl Iterator<Item = &Job> {
        self.jobs.iter().filter(|j| !j.done)
    }
}

fn notify_done(ok: bool, title: &str) {
    let summary = if ok {
        "MMC job completed"
    } else {
        "MMC job failed"
    };
    let _ = std::process::Command::new("notify-send")
        .args(["-a", "mde-mmc", summary, title])
        .status();
}
