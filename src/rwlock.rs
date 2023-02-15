//! PThread read-write lock implemented using libctru.

use crate::{condvar, mutex};

struct rwlock_clear {
    mutex: libc::pthread_mutex_t,
    cvar: i32,
    num_readers: i32,
    writer_active: bool,
    initialized: bool,
}

/// Initializes the rwlock internal members if they weren't already
fn init_rwlock(lock: *mut libc::pthread_rwlock_t) {
    let lock = lock as *mut rwlock_clear;

    unsafe {
        if !(*lock).initialized {
            let mut attr = std::mem::MaybeUninit::<libc::pthread_mutexattr_t>::uninit();
            mutex::pthread_mutexattr_init(attr.as_mut_ptr());
            let mut attr = attr.assume_init();
            mutex::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_NORMAL);

            mutex::pthread_mutex_init(&mut (*lock).mutex, &attr);
            condvar::pthread_cond_init(&mut (*lock).cvar as *mut i32 as *mut _, core::ptr::null());

            (*lock).num_readers = 0;
            (*lock).writer_active = false;

            (*lock).initialized = true;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_init(
    lock: *mut libc::pthread_rwlock_t,
    _attr: *const libc::pthread_rwlockattr_t,
) -> libc::c_int {
    init_rwlock(lock);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_destroy(_lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_rdlock(lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    mutex::pthread_mutex_lock(&mut (*lock).mutex);

    while (*lock).writer_active {
        condvar::pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).num_readers += 1;

    mutex::pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_tryrdlock(
    lock: *mut libc::pthread_rwlock_t,
) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    if mutex::pthread_mutex_trylock(&mut (*lock).mutex) != 0 {
        return -1;
    }

    while (*lock).writer_active {
        condvar::pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).num_readers += 1;

    mutex::pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_wrlock(lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    mutex::pthread_mutex_lock(&mut (*lock).mutex);

    while (*lock).writer_active || (*lock).num_readers > 0 {
        condvar::pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).writer_active = true;

    mutex::pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_trywrlock(
    lock: *mut libc::pthread_rwlock_t,
) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    if mutex::pthread_mutex_trylock(&mut (*lock).mutex) != 0 {
        return -1;
    }

    while (*lock).writer_active || (*lock).num_readers > 0 {
        condvar::pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).writer_active = true;

    mutex::pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_unlock(lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    mutex::pthread_mutex_lock(&mut (*lock).mutex);

    // If there are readers and no writer => Must be a reader
    if (*lock).num_readers > 0 && !(*lock).writer_active {
        (*lock).num_readers -= 1;

        // If there are no more readers, signal to a waiting writer
        if (*lock).num_readers == 0 {
            condvar::pthread_cond_signal(&mut (*lock).cvar as *mut i32 as _);
        }
    // If there are no readers and a writer => Must be a writer
    } else if (*lock).num_readers == 0 && (*lock).writer_active {
        (*lock).writer_active = false;

        condvar::pthread_cond_broadcast(&mut (*lock).cvar as *mut i32 as _);
    }

    mutex::pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlockattr_init(
    _attr: *mut libc::pthread_rwlockattr_t,
) -> libc::c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlockattr_destroy(
    _attr: *mut libc::pthread_rwlockattr_t,
) -> libc::c_int {
    0
}
