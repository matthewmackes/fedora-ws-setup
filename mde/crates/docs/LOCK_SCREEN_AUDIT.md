# Lock screen Windows 11 look / feel / layout / security audit

**Date:** 2026-11-21  
**Scope:** Protocol lock (`mde-lock` + anvil `ext-session-lock`), soft iced overlay fallback (`mde-shell`), Settings › Lock screen, Spotlight → lock wallpaper path.  
**Reference:** Windows 11 lock / sign-in face (clock → swipe/dismiss → password), MDE greeter (`mde-greeter`), fidelity tokens, worklist L2 / W21–W429 / A054–A055.  
**Method:** Static inspection of current tree. No interactive screenshot / SSIM pass in this audit (`mde/fidelity/specs/lock.toml` remains `unmeasured`).

---

## Surfaces inventoried

| Surface | Role today | Toolkit / protocol |
|---------|------------|--------------------|
| `mde-greeter` → `lock.rs` (`mde-lock` bin) | **Primary** lock UI when `ext-session-lock-v1` works | SCTK + raw ARGB bitmap (5×7 font) |
| `mde-anvil` `session_lock.rs` | Compositor lock manager | smithay `SessionLockHandler` |
| `mde-shell` soft overlay (`view_overlay`) | Fallback when `mde-lock` / loginctl unavailable | iced layer overlay |
| `mde-shell` `idle.rs` + `engage_lock` | Idle timer, Super+L, Start lock, respawn | shell-owned |
| `mde-control` › Personalization › Lock screen | Settings hub | iced status/link rows |
| `mde-control` › Background Spotlight toggles | Writes `$XDG_RUNTIME_DIR/mde-lock-wallpaper.txt` | **not consumed by lock** |
| Legacy `lib/ws-greeter/lock.py` / swaylock | Install-path / LightDM era | out of primary Rust path |

Login greeter (`mde-greeter` iced) is a **sibling** surface (L1), not the lock face. Caps Lock / a11y / layout polish on the greeter does **not** transfer to `mde-lock`.

---

## Executive verdict

| Area | Protocol lock (`mde-lock`) | Soft iced fallback |
|------|----------------------------|--------------------|
| Look (type, wallpaper, materials) | Weak — bitmap font, solid navy, no real wallpaper/blur | Partial — Selawik-ish shell type; no wallpaper |
| Feel (auth, media, switch user) | Functional PAM + transport hits; many tips are stubs | **Unlock without password** |
| Layout (clock → user → field → actions) | Structural Win11 stack present | Structural; missing password field |
| Security | PAM `login` on primary path | Soft unlock is click-to-dismiss |
| Settings / Spotlight wiring | Idle delay only; wallpaper file unread | N/A |
| Fidelity | Spec scaffold only (`unmeasured`) | Same |

**Bottom line:** Lock **capability** (ext-session-lock + PAM + idle/Super+L + media/switch-user) is real and largely marked done in the worklist. Win11 **visual fidelity** and Settings depth are thin: the primary face is a hand-drawn bitmap UI that never loads wallpaper, diverges from the soft overlay and greeter, and several “done” claims are tip text or dead wires (Spotlight lock path, slideshow tip, swipe hint). Soft lock is a security regression vs PAM when the protocol client is missing.

---

## 1. Look (theme, type, materials)

### Win11 expectations

- Full-bleed wallpaper (often blurred / dimmed)
- Large Segoe-style clock + long date
- Acrylic / acrylic-ish sign-in pill; circular avatar photo
- Accent used sparingly; media card with art

### `mde-lock` today

| Finding | Evidence | Severity |
|---------|----------|----------|
| No wallpaper / blur — solid `#0a1222` + procedural vignette + accent edge glow | `draw_lock_frame` fills canvas then shades; comment claims W140/W160 “blur wallpaper” but no image decode | **High** |
| Spotlight lock path is **write-only** | `community::record_spotlight` writes `mde-lock-wallpaper.txt`; **zero** reads in `mde-lock` / shell soft lock | **High** (A054 dead wire) |
| 5×7 bitmap font, not Selawik / type ramp | `glyph` / `draw_text` in `lock.rs` | **High** |
| Avatar is initial circle only (no `~/.face`) | W41 path; W240 avatar image only on **soft** lock | Medium |
| Hardcoded ARGB colours; ignores `Palette::load` / light-dark / HC | Contrast to greeter which loads accent | Medium |
| 24h `HH:MM:SS` clock vs soft lock 12h `%-I:%M %p` | `format_clock` vs `clock_time()` | Low–Medium |
| “Slideshow · use Settings” tip with no slideshow engine on lock | W260 text only | Low |

### Soft overlay today

Uses iced text sizes (56 / 16 / …) and shell chrome helpers (`flyout_inset`, `accent_pill`) — closer to MDE type language — but still no wallpaper background and shows a fallback banner (`Soft lock · mde-lock … unavailable`).

---

## 2. Feel (interaction / auth)

### Win11 expectations

- Dismiss / swipe up → password (or PIN / Hello)
- Caps Lock indicator; Ease of Access; power / network status often present
- Media transport when playing
- Switch user without unlocking into another session where multi-user exists
- Esc does not unlock a real session

### Protocol lock

| Expectation | Status |
|-------------|--------|
| PAM password unlock | **Real** — `pam_auth::authenticate_user` / service `login` |
| Enter / Unlock button | Works |
| Esc unlock | Only if `MDE_LOCK_UI_ONLY=1` or `MDE_SAFE` — good |
| Esc clears field otherwise | Yes |
| Caps Lock indicator | **Absent** (greeter has W229; lock fidelity state `caps` unmeasured/empty) |
| Swipe-up dismiss | **Hint only** (“Swipe up to unlock · Press Esc”) — no gesture handler (W180) |
| Media prev / play-pause / next | Hit targets + `playerctl` (W429 / A055) |
| Media art / volume / progress | Soft lock richer; `mde-lock` letter-tile + ASCII labels |
| Switch user | Terminates session → greetd (W428); honest seatd limit |
| Network / battery / power / Ease of Access | Greeter has stubs; lock face does not |
| Unlock via `$XDG_RUNTIME_DIR/mde-unlock.req` | Polled every 100 ms; any same-UID process can unlock protocol lock | Medium (session-local threat) |

### Soft overlay — critical gap

| Finding | Evidence | Severity |
|---------|----------|----------|
| **Unlock requires no password** | `Message::Unlock` → `idle.unlock()` + optional `write_unlock_req`; button is `on_press(Message::Unlock)` with no field | **Critical** when this path is the live lock |
| No password field at all | `view_overlay` lock branch | High (feel + security) |
| Shown whenever `Locked && !protocol_lock_active()` | Includes spawn failure / loginctl-only cases | High |

Primary path is PAM-backed; treat soft overlay as **insecure fallback**, not Win11 parity.

---

## 3. Layout

### Structural (protocol lock) — matches Win11 stack

1. Large clock + date (upper third)  
2. Weather + refresh  
3. Optional Now playing card  
4. Avatar + username  
5. Password field + Unlock  
6. Switch user + hints / tips  

Spacing is absolute pixel math (`UnlockButton::centered`, `clock_y = height/4`), not fidelity `space` / `type` tokens. Multi-monitor: lock surfaces created for outputs known at start; **hotplug deferred** (`new_output` comment).

### Soft overlay layout gaps

- No password row  
- Media card above Unlock (no sign-in field)  
- Fallback status line makes it look like a debug overlay, not a product face  

### Settings › Lock screen (`personal::lockscreen`)

W339 claimed “preview, background source dropdown, lock delay linked to idle lock”. Actual page:

- Status: idle seconds (read-only here; edit lives under Power)  
- Status: Switch user seat limit note  
- Status: wallpaper path string  
- Links: Background, Power & battery  

**Missing vs claim:** preview, background-source dropdown, delay control on this page. Severity: Medium (discoverability / honesty of “done”).

---

## 4. Dual-face divergence

Primary and fallback are not the same product:

| Feature | `mde-lock` | Soft overlay |
|---------|------------|--------------|
| Auth | PAM | Click Unlock |
| Type | Bitmap 5×7 | iced / theme fonts |
| Avatar image | Initial only | `~/.face` / avatar.png |
| Media | Text “Prev Play/Pause Next” | Emoji transport + vol + progress |
| Weather refresh | Yes | Yes |
| Switch user | Yes | No |
| Wallpaper | No | No |
| Tips / slideshow copy | Yes | Slideshow tip only |

Any polish on one face that is not shared will regress when the other engages.

---

## 5. Security & session integrity (positive + gaps)

### What works

- Compositor `ext-session-lock-v1` blanks/focuses lock surfaces (`session_lock.rs`)
- Nested lock rejected
- Shell respawns crashed `mde-lock` while flag set; success exit can unlock via supervisor policy
- Orphan `mde-lock` cleanup / session `unlock` helpers
- Share (screencast) stopped on lock (W449)
- Optional clipboard clear-on-lock
- Password buffer zeroed after PAM attempt
- Fullscreen idle inhibit (L3)

### Gaps

1. Soft overlay unlock without credentials (**Critical** if fallback is reachable on a real seat)  
2. `mde-unlock.req` as unlock oracle for same-UID processes (**Medium**)  
3. Misleading Esc / swipe copy on a PAM lock (**Low** UX, can train bad habits)  

---

## 6. Worklist honesty check

| ID | Claimed | Audit note |
|----|---------|------------|
| L2 / W2 | Secure ext-session-lock + PAM | **True** on primary path |
| W21 / W140 / W160 | Wallpaper dim / blur polish | Vignette on solid fill only — **overstated** |
| W41 / W240 | Avatar | Split: image on soft only |
| W79 / W260 | Tips / slideshow | Copy present; slideshow not real |
| W180 | Dismiss drag hint | Hint without gesture |
| W339 | Settings preview + source dropdown | **Not present** — status/links only |
| W428 / W429 / A055 | Switch user + media | Present; media polish stronger on soft lock |
| A054 | Spotlight on lock | File written; **lock never reads it** |

Fidelity: `lock.toml` states `clock` / `password` / `caps` with empty refs — visual parity unmeasured.

---

## 7. Priority gap list

Ordered by impact on “does this feel / act like a Win11 lock”:

1. **Close soft-lock security hole** — require PAM (or refuse soft unlock / keep seat blank) when protocol lock is down  
2. **Load real wallpaper** on `mde-lock` (desktop / Spotlight / `mde-lock-wallpaper.txt`) + real blur/dim  
3. **Replace bitmap font** with Selawik (or share greeter iced face under session-lock) so lock ≈ greeter  
4. **Unify faces** — one lock UI module; soft overlay must not diverge on auth/avatar/media  
5. **Wire Caps Lock** (and ideally network/power) on the lock face — fidelity `caps` state  
6. **Settings › Lock screen** — preview + background source + idle delay (honest W339)  
7. Implement or remove **swipe / slideshow** stubs  
8. Soften / gate **`mde-unlock.req`** (socket to shell, or PAM-only unlock)  
9. Capture **fidelity** refs for `lock.toml` (clock / password / caps)  
10. Multi-monitor **hotplug** lock surfaces  

---

## Evidence index

| Claim | Where verified |
|-------|----------------|
| Primary UI is bitmap ARGB | `mde-greeter/src/lock.rs` `draw_lock_frame` / `glyph` |
| PAM unlock | `pam_auth.rs`; Enter / Unlock → `try_pam_unlock` |
| Soft unlock no password | `mde-shell` `Message::Unlock` + overlay Unlock button |
| Spotlight file unread | `community::record_spotlight` write; no `mde-lock-wallpaper` readers |
| Settings lock page thin | `mde-control/src/pages/personal.rs` `lockscreen` |
| Compositor lock | `mde-anvil/src/session_lock.rs` |
| Fidelity unmeasured | `mde/fidelity/specs/lock.toml` |
| Idle / Super+L engage | `engage_lock`, anvil `KeyAction::Lock` |

---

## Audit status

**Initial audit:** complete (look / feel / layout / dual-face / security / Settings / worklist honesty).  
**Still open:** interactive screenshot / SSIM against `lock.toml`; live soft-fallback repro on a seat without `mde-lock`.
