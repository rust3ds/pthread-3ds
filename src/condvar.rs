//! PThread condition variables implemented using libctru.

pub fn init() {}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_init(
    cond: *mut libc::pthread_cond_t,
    _attr: *const libc::pthread_condattr_t,
) -> libc::c_int {
    ctru_sys::CondVar_Init(cond as _);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_signal(cond: *mut libc::pthread_cond_t) -> libc::c_int {
    ctru_sys::CondVar_WakeUp(cond as _, 1);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_broadcast(cond: *mut libc::pthread_cond_t) -> libc::c_int {
    ctru_sys::CondVar_WakeUp(cond as _, -1);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_wait(
    cond: *mut libc::pthread_cond_t,
    lock: *mut libc::pthread_mutex_t,
) -> libc::c_int {
    ctru_sys::CondVar_Wait(cond as _, lock as _);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_timedwait(
    cond: *mut libc::pthread_cond_t,
    lock: *mut libc::pthread_mutex_t,
    abstime: *const libc::timespec,
) -> libc::c_int {
    // libctru expects a duration, but we have an absolute timestamp.
    // Convert to a duration before calling libctru.

    // Get the current time so we can make a duration
    let mut now = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let r = libc::gettimeofday(&mut now, std::ptr::null_mut());
    if r != 0 {
        return r;
    }

    // Calculate the duration
    let duration_nsec = (*abstime)
        .tv_sec
        // Get the difference in seconds
        .saturating_sub(now.tv_sec)
        // Convert to nanoseconds
        .saturating_mul(1_000_000_000)
        // Add the difference in nanoseconds
        .saturating_add((*abstime).tv_nsec as i64)
        .saturating_sub(now.tv_usec as i64 * 1_000)
        // Don't go negative
        .max(0);

    let r = ctru_sys::CondVar_WaitTimeout(cond as _, lock as _, duration_nsec);

    // CondVar_WaitTimeout returns a boolean which is true (nonzero) if it timed out
    if r == 0 {
        0
    } else {
        libc::ETIMEDOUT
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_destroy(_cond: *mut libc::pthread_cond_t) -> libc::c_int {
    0
}

#[no_mangle]
pub extern "C" fn pthread_condattr_init(_attr: *const libc::pthread_condattr_t) -> libc::c_int {
    0
}

#[no_mangle]
pub extern "C" fn pthread_condattr_destroy(_attr: *const libc::pthread_condattr_t) -> libc::c_int {
    0
}

#[no_mangle]
pub extern "C" fn pthread_condattr_getclock(
    _attr: *const libc::pthread_condattr_t,
    clock_id: *mut libc::clockid_t,
) -> libc::c_int {
    unsafe {
        *clock_id = libc::CLOCK_REALTIME;
    }

    0
}

#[no_mangle]
pub extern "C" fn pthread_condattr_setclock(
    _attr: *mut libc::pthread_condattr_t,
    clock_id: libc::clockid_t,
) -> libc::c_int {
    // only one clock is supported, so all other options are considered an error
    if clock_id == libc::CLOCK_REALTIME {
        0
    } else {
        libc::EINVAL
    }
}
