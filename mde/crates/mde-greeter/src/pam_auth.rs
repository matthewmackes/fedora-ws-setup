//! Minimal libpam client for `mde-lock` (no bindgen / libclang).

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Mutex;

const PAM_SUCCESS: c_int = 0;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_ERROR_MSG: c_int = 3;
const PAM_TEXT_INFO: c_int = 4;

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

#[repr(C)]
struct PamConv {
    conv: Option<
        unsafe extern "C" fn(
            num_msg: c_int,
            msg: *mut *const PamMessage,
            resp: *mut *mut PamResponse,
            appdata_ptr: *mut c_void,
        ) -> c_int,
    >,
    appdata_ptr: *mut c_void,
}

#[repr(C)]
struct PamHandle {
    _private: [u8; 0],
}

// Linked via build.rs (`-lpam` or `-l:libpam.so.0`).
unsafe extern "C" {
    fn pam_start(
        service_name: *const c_char,
        user: *const c_char,
        pam_conversation: *const PamConv,
        pamh: *mut *mut PamHandle,
    ) -> c_int;
    fn pam_authenticate(pamh: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_acct_mgmt(pamh: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut PamHandle, pam_status: c_int) -> c_int;
    fn pam_strerror(pamh: *mut PamHandle, errnum: c_int) -> *const c_char;
}

static CONV_PASSWORD: Mutex<Option<String>> = Mutex::new(None);

unsafe extern "C" fn conversation(
    num_msg: c_int,
    msg: *mut *const PamMessage,
    resp: *mut *mut PamResponse,
    _appdata_ptr: *mut c_void,
) -> c_int {
    if num_msg <= 0 || msg.is_null() || resp.is_null() {
        return 1;
    }
    let password = CONV_PASSWORD
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    let replies = libc::calloc(num_msg as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
    if replies.is_null() {
        return 1;
    }
    for i in 0..num_msg as usize {
        let message = &*(*msg.add(i));
        let slot = &mut *replies.add(i);
        slot.resp_retcode = 0;
        slot.resp = ptr::null_mut();
        match message.msg_style {
            PAM_PROMPT_ECHO_OFF | PAM_PROMPT_ECHO_ON => {
                if let Ok(c) = CString::new(password.as_str()) {
                    slot.resp = libc::strdup(c.as_ptr());
                }
            }
            PAM_ERROR_MSG | PAM_TEXT_INFO => {}
            _ => {}
        }
    }
    *resp = replies;
    PAM_SUCCESS
}

/// Authenticate `user` with `password` via PAM service `login` (Fedora stock).
pub fn authenticate_user(user: &str, password: &str) -> Result<(), String> {
    if user.is_empty() {
        return Err("empty user".into());
    }
    let service = CString::new("login").map_err(|_| "bad service".to_string())?;
    let user_c = CString::new(user).map_err(|_| "bad user".to_string())?;
    if let Ok(mut guard) = CONV_PASSWORD.lock() {
        *guard = Some(password.to_string());
    }
    let conv = PamConv {
        conv: Some(conversation),
        appdata_ptr: ptr::null_mut(),
    };
    let mut pamh: *mut PamHandle = ptr::null_mut();
    let rc = unsafe { pam_start(service.as_ptr(), user_c.as_ptr(), &conv, &mut pamh) };
    if rc != PAM_SUCCESS {
        clear_password();
        return Err(format!("pam_start: {rc}"));
    }
    let rc_auth = unsafe { pam_authenticate(pamh, 0) };
    let rc_acct = if rc_auth == PAM_SUCCESS {
        unsafe { pam_acct_mgmt(pamh, 0) }
    } else {
        rc_auth
    };
    let err = if rc_acct != PAM_SUCCESS {
        let msg = unsafe { pam_strerror(pamh, rc_acct) };
        let text = if msg.is_null() {
            format!("PAM error {rc_acct}")
        } else {
            unsafe { CStr::from_ptr(msg).to_string_lossy().into_owned() }
        };
        Some(text)
    } else {
        None
    };
    unsafe {
        pam_end(pamh, rc_acct);
    }
    clear_password();
    match err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn clear_password() {
    if let Ok(mut guard) = CONV_PASSWORD.lock() {
        if let Some(ref mut s) = *guard {
            for b in unsafe { s.as_bytes_mut() } {
                *b = 0;
            }
        }
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_empty_user() {
        assert!(super::authenticate_user("", "x").is_err());
    }
}
