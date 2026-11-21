"""Password check through PAM (the swaylock service), without extra Python packages."""

from ctypes import (CDLL, CFUNCTYPE, POINTER, Structure, byref, c_char_p, c_int, c_size_t, c_void_p,
                    cast, sizeof)
from ctypes.util import find_library

PROMPT_ECHO_OFF, PROMPT_ECHO_ON = 1, 2
SUCCESS, REFRESH_CRED = 0, 0x0010


class Message(Structure):
    _fields_ = [("msg_style", c_int), ("msg", c_char_p)]


class Response(Structure):
    _fields_ = [("resp", c_void_p), ("resp_retcode", c_int)]


CONV = CFUNCTYPE(c_int, c_int, POINTER(POINTER(Message)), POINTER(POINTER(Response)), c_void_p)


class Conv(Structure):
    _fields_ = [("conv", CONV), ("appdata_ptr", c_void_p)]


_libpam = CDLL(find_library("pam") or "libpam.so.0")
_libc = CDLL(find_library("c") or "libc.so.6")
_libc.calloc.restype = c_void_p
_libc.calloc.argtypes = [c_size_t, c_size_t]
_libc.strdup.restype = c_void_p
_libc.strdup.argtypes = [c_char_p]
_libpam.pam_start.argtypes = [c_char_p, c_char_p, POINTER(Conv), POINTER(c_void_p)]
_libpam.pam_authenticate.argtypes = [c_void_p, c_int]
_libpam.pam_setcred.argtypes = [c_void_p, c_int]
_libpam.pam_end.argtypes = [c_void_p, c_int]
_libpam.pam_strerror.argtypes = [c_void_p, c_int]
_libpam.pam_strerror.restype = c_char_p


def authenticate(user, password, service="swaylock"):
    """Returns (ok, message)."""
    secret = password.encode()

    def conversation(n, messages, responses, _data):
        block = _libc.calloc(n, sizeof(Response))
        if not block:
            return 5  # PAM_BUF_ERR
        replies = cast(block, POINTER(Response))
        for i in range(n):
            if messages[i].contents.msg_style in (PROMPT_ECHO_OFF, PROMPT_ECHO_ON):
                replies[i].resp = _libc.strdup(secret)
        responses[0] = replies
        return SUCCESS

    conv = Conv(CONV(conversation), None)
    handle = c_void_p()
    ret = _libpam.pam_start(service.encode(), user.encode(), byref(conv), byref(handle))
    if ret != SUCCESS:
        return False, "Couldn't start authentication."
    ret = _libpam.pam_authenticate(handle, 0)
    if ret == SUCCESS:
        _libpam.pam_setcred(handle, REFRESH_CRED)
    message = (_libpam.pam_strerror(handle, ret) or b"").decode()
    _libpam.pam_end(handle, ret)
    return ret == SUCCESS, message
