//! Miscellaneous pthread functions

// The implementation within `newlib` stubs this out *entirely*. It's not possible to use a "syscall".
/*
#[no_mangle]
pub unsafe extern "C" fn sched_yield() -> libc::c_int {
    ctru_sys::svcSleepThread(0);

    0
}
*/

// `pthread_sigmask` and `pthread_atfork` are stubbed out by `newlib`