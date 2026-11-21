//! Virtual desktops (Win11 Task View parity).
//!
//! Inactive-desktop windows are unmapped from the space and kept in `parked`
//! with their last location so they can be remapped on switch.

use std::cell::RefCell;

use smithay::{
    desktop::Space,
    utils::{IsAlive, Logical, Point},
};

use super::WindowElement;

pub const MAX_WORKSPACES: usize = 8;
pub const DEFAULT_WORKSPACE_COUNT: usize = 2;

/// Per-window desktop membership (0-based).
#[derive(Debug, Default)]
pub struct WindowWorkspace(RefCell<usize>);

impl WindowWorkspace {
    pub fn get(&self) -> usize {
        *self.0.borrow()
    }

    pub fn set(&self, idx: usize) {
        *self.0.borrow_mut() = idx;
    }
}

#[derive(Debug)]
pub struct ParkedWindow {
    pub window: WindowElement,
    pub loc: Point<i32, Logical>,
}

#[derive(Debug)]
pub struct Workspaces {
    pub active: usize,
    pub count: usize,
    pub parked: Vec<ParkedWindow>,
}

impl Default for Workspaces {
    fn default() -> Self {
        Self {
            active: 0,
            count: DEFAULT_WORKSPACE_COUNT,
            parked: Vec::new(),
        }
    }
}

impl Workspaces {
    pub fn tag_window(&self, window: &WindowElement, workspace: usize) {
        let ws = workspace.min(self.count.saturating_sub(1));
        window
            .user_data()
            .insert_if_missing(WindowWorkspace::default);
        if let Some(slot) = window.user_data().get::<WindowWorkspace>() {
            slot.set(ws);
        }
    }

    pub fn window_workspace(window: &WindowElement) -> usize {
        window
            .user_data()
            .get::<WindowWorkspace>()
            .map(|w| w.get())
            .unwrap_or(0)
    }

    pub fn prune_dead(&mut self) {
        self.parked.retain(|p| p.window.alive());
    }

    /// Every known toplevel: mapped in `space` plus parked on other desktops.
    pub fn all_windows(&self, space: &Space<WindowElement>) -> Vec<WindowElement> {
        let mut out: Vec<WindowElement> = space.elements().cloned().collect();
        for p in &self.parked {
            if p.window.alive() && !out.iter().any(|w| w == &p.window) {
                out.push(p.window.clone());
            }
        }
        out
    }

    pub fn switch_to(&mut self, space: &mut Space<WindowElement>, target: usize) {
        self.prune_dead();
        if self.count == 0 {
            self.count = 1;
        }
        let target = target.min(self.count - 1);
        if target == self.active {
            return;
        }

        // Park windows that belong on the desktop we are leaving.
        let mapped: Vec<WindowElement> = space.elements().cloned().collect();
        for window in mapped {
            let ws = Self::window_workspace(&window);
            if ws != self.active {
                // Should not happen; re-tag to active if somehow mismatched.
                self.tag_window(&window, self.active);
            }
            let loc = space
                .element_location(&window)
                .unwrap_or_else(|| Point::from((0, 0)));
            space.unmap_elem(&window);
            self.parked.push(ParkedWindow { window, loc });
        }

        self.active = target;

        // Remap windows that belong on the new active desktop.
        let mut stay = Vec::new();
        let mut restore = Vec::new();
        for p in self.parked.drain(..) {
            if !p.window.alive() {
                continue;
            }
            if Self::window_workspace(&p.window) == self.active {
                restore.push(p);
            } else {
                stay.push(p);
            }
        }
        self.parked = stay;
        for p in restore {
            space.map_element(p.window, p.loc, false);
        }
    }

    pub fn next(&mut self, space: &mut Space<WindowElement>) {
        if self.count <= 1 {
            return;
        }
        let target = (self.active + 1) % self.count;
        self.switch_to(space, target);
    }

    pub fn prev(&mut self, space: &mut Space<WindowElement>) {
        if self.count <= 1 {
            return;
        }
        let target = if self.active == 0 {
            self.count - 1
        } else {
            self.active - 1
        };
        self.switch_to(space, target);
    }

    pub fn new_desktop(&mut self, space: &mut Space<WindowElement>) -> bool {
        if self.count >= MAX_WORKSPACES {
            return false;
        }
        self.count += 1;
        let target = self.count - 1;
        self.switch_to(space, target);
        true
    }

    /// Close the current desktop; windows merge into the previous (or next if first).
    pub fn close_current(&mut self, space: &mut Space<WindowElement>) -> bool {
        self.prune_dead();
        if self.count <= 1 {
            return false;
        }

        let closing = self.active;
        let merge_into = if closing == 0 { 1 } else { closing - 1 };

        let retag = |w: &WindowElement| {
            let ws = Self::window_workspace(w);
            if ws == closing {
                if let Some(slot) = w.user_data().get::<WindowWorkspace>() {
                    slot.set(merge_into);
                }
            }
        };
        for w in space.elements() {
            retag(w);
        }
        for p in &self.parked {
            retag(&p.window);
        }

        // Renumber desktops above the closed index down by one.
        let renumber = |w: &WindowElement| {
            let ws = Self::window_workspace(w);
            if ws > closing {
                if let Some(slot) = w.user_data().get::<WindowWorkspace>() {
                    slot.set(ws - 1);
                }
            }
        };
        for w in space.elements() {
            renumber(w);
        }
        for p in &self.parked {
            renumber(&p.window);
        }

        self.count -= 1;
        let new_active = if closing == 0 { 0 } else { closing - 1 }.min(self.count - 1);

        // Rematerialize so mapped set matches membership after renumber.
        self.rematerialize(space, new_active);
        true
    }

    /// Park every mapped window, then restore those tagged for `target`.
    fn rematerialize(&mut self, space: &mut Space<WindowElement>, target: usize) {
        let mapped: Vec<WindowElement> = space.elements().cloned().collect();
        for window in mapped {
            let loc = space
                .element_location(&window)
                .unwrap_or_else(|| Point::from((0, 0)));
            space.unmap_elem(&window);
            self.parked.push(ParkedWindow { window, loc });
        }
        self.active = target.min(self.count.saturating_sub(1));

        let mut stay = Vec::new();
        let mut restore = Vec::new();
        for p in self.parked.drain(..) {
            if !p.window.alive() {
                continue;
            }
            if Self::window_workspace(&p.window) == self.active {
                restore.push(p);
            } else {
                stay.push(p);
            }
        }
        self.parked = stay;
        for p in restore {
            space.map_element(p.window, p.loc, false);
        }
    }

    pub fn move_window_to(
        &mut self,
        space: &mut Space<WindowElement>,
        window: &WindowElement,
        target: usize,
    ) {
        self.prune_dead();
        if self.count == 0 {
            return;
        }
        let target = target.min(self.count - 1);
        let current = Self::window_workspace(window);
        if current == target {
            return;
        }
        self.tag_window(window, target);

        if target == self.active {
            // Bring onto this desktop if parked.
            if let Some(idx) = self.parked.iter().position(|p| &p.window == window) {
                let p = self.parked.remove(idx);
                space.map_element(p.window, p.loc, true);
            }
            return;
        }

        // Leaving the active desktop: park it.
        if space.elements().any(|w| w == window) {
            let loc = space
                .element_location(window)
                .unwrap_or_else(|| Point::from((0, 0)));
            space.unmap_elem(window);
            self.parked.push(ParkedWindow {
                window: window.clone(),
                loc,
            });
        } else if let Some(p) = self.parked.iter_mut().find(|p| &p.window == window) {
            // Already parked; membership updated via tag.
            let _ = p;
        }
    }
}
