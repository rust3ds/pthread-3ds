//! Miscellaneous pthread functions

#[no_mangle]
pub unsafe extern "C" fn sched_yield() -> libc::c_int {
    ctru_sys::svcSleepThread(0);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_sigmask(
    _how: ::libc::c_int,
    _set: *const libc::sigset_t,
    _oldset: *mut libc::sigset_t,
) -> ::libc::c_int {
    -1
}

#[no_mangle]
pub extern "C" fn pthread_atfork(
    _prepare: Option<unsafe extern "C" fn()>,
    _parent: Option<unsafe extern "C" fn()>,
    _child: Option<unsafe extern "C" fn()>,
) -> libc::c_int {
    0
}
