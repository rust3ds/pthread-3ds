//! PThread mutex implemented using libctru.

#[no_mangle]
pub unsafe extern "C" fn pthread_mutexattr_init(
    _attr: *mut libc::pthread_mutexattr_t,
) -> libc::c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutexattr_settype(
    attr: *mut libc::pthread_mutexattr_t,
    _type: libc::c_int,
) -> libc::c_int {
    let attr: *mut libc::c_int = attr as _;

    *attr = _type as _;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_init(
    lock: *mut libc::pthread_mutex_t,
    attr: *const libc::pthread_mutexattr_t,
) -> libc::c_int {
    let lock = lock as *mut u8;

    let attr: libc::c_int = *(attr as *const libc::c_int);

    if attr == libc::PTHREAD_MUTEX_NORMAL {
        ctru_sys::LightLock_Init(lock as _);
    } else if attr == libc::PTHREAD_MUTEX_RECURSIVE {
        ctru_sys::RecursiveLock_Init(lock as _)
    }

    *(lock.offset(39)) = attr as u8;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_destroy(_lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_lock(lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    let lock = lock as *const u8;

    if *(lock.offset(39)) == libc::PTHREAD_MUTEX_NORMAL as u8 {
        ctru_sys::LightLock_Lock(lock as _);
    } else if *(lock.offset(39)) == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        ctru_sys::RecursiveLock_Lock(lock as _);
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_trylock(lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    let lock = lock as *const u8;

    if *(lock.offset(39)) == libc::PTHREAD_MUTEX_NORMAL as u8 {
        return ctru_sys::LightLock_TryLock(lock as _);
    } else if *(lock.offset(39)) == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        return ctru_sys::RecursiveLock_TryLock(lock as _);
    }

    -1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_unlock(lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    let lock = lock as *const u8;

    if *(lock.offset(39)) == libc::PTHREAD_MUTEX_NORMAL as u8 {
        ctru_sys::LightLock_Unlock(lock as _);
    } else if *(lock.offset(39)) == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        ctru_sys::RecursiveLock_Unlock(lock as _);
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutexattr_destroy(
    _attr: *mut libc::pthread_mutexattr_t,
) -> libc::c_int {
    0
}
