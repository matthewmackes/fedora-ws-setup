//! Soft idle / lock (shell-owned iced overlay fallback; prefers `mde-lock` / ext-session-lock).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockPhase {
    Unlocked,
    IdleWarning,
    Locked,
}

#[derive(Debug)]
pub struct IdleLock {
    pub lock_after_secs: u64,
    pub phase: LockPhase,
    idle_secs: u64,
}

impl IdleLock {
    pub fn new(lock_after_secs: u64) -> Self {
        Self {
            lock_after_secs: lock_after_secs.max(1),
            phase: LockPhase::Unlocked,
            idle_secs: 0,
        }
    }

    pub fn tick_idle(&mut self, secs: u64) -> LockPhase {
        if self.phase == LockPhase::Locked {
            return self.phase;
        }
        self.idle_secs = self.idle_secs.saturating_add(secs);
        let warn_at = self.lock_after_secs.saturating_sub(10).max(1);
        if self.idle_secs >= self.lock_after_secs {
            self.phase = LockPhase::Locked;
        } else if self.idle_secs >= warn_at {
            self.phase = LockPhase::IdleWarning;
        }
        self.phase
    }

    pub fn activity(&mut self) {
        self.idle_secs = 0;
        if self.phase != LockPhase::Locked {
            self.phase = LockPhase::Unlocked;
        }
    }

    pub fn lock_now(&mut self) {
        self.idle_secs = self.lock_after_secs;
        self.phase = LockPhase::Locked;
    }

    pub fn unlock(&mut self) {
        self.idle_secs = 0;
        self.phase = LockPhase::Unlocked;
    }

    /// Apply a new idle timeout from config reload without forcing a lock.
    pub fn set_lock_after_secs(&mut self, secs: u64) {
        self.lock_after_secs = secs.max(1);
        if self.phase != LockPhase::Locked && self.idle_secs < self.lock_after_secs {
            let warn_at = self.lock_after_secs.saturating_sub(10).max(1);
            self.phase = if self.idle_secs >= warn_at {
                LockPhase::IdleWarning
            } else {
                LockPhase::Unlocked
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaches_locked() {
        let mut i = IdleLock::new(30);
        assert_eq!(i.tick_idle(25), LockPhase::IdleWarning);
        assert_eq!(i.tick_idle(10), LockPhase::Locked);
        assert_eq!(i.tick_idle(5), LockPhase::Locked);
        i.unlock();
        assert_eq!(i.phase, LockPhase::Unlocked);
    }
}
