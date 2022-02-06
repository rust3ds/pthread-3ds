#![feature(thread_local)]
#![feature(const_btree_new)]
#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

/// Call this somewhere to force Rust to link this module.
/// The call doesn't need to execute, just exist.
///
/// See https://github.com/rust-lang/rust/issues/47384
pub fn init() {}

// LIBCTRU THREADS

pub type _LOCK_T = i32;
pub type _LOCK_RECURSIVE_T = __lock_t;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __lock_t {
    pub lock: _LOCK_T,
    pub thread_tag: u32,
    pub counter: u32,
}

type LightLock = _LOCK_T;
type RecursiveLock = _LOCK_RECURSIVE_T;
type CondVar = i32;

extern "C" {
    fn LightLock_Init(lock: *mut LightLock);
    fn LightLock_Lock(lock: *mut LightLock);
    fn LightLock_TryLock(lock: *mut LightLock) -> libc::c_int;
    fn LightLock_Unlock(lock: *mut LightLock);

    fn RecursiveLock_Init(lock: *mut RecursiveLock);
    fn RecursiveLock_Lock(lock: *mut RecursiveLock);
    fn RecursiveLock_TryLock(lock: *mut RecursiveLock) -> libc::c_int;
    fn RecursiveLock_Unlock(lock: *mut RecursiveLock);

    fn CondVar_Init(cv: *mut CondVar);
    fn CondVar_Wait(cv: *mut CondVar, lock: *mut LightLock);
    fn CondVar_WaitTimeout(cv: *mut CondVar, lock: *mut LightLock, timeout_ns: i64) -> libc::c_int;
    fn CondVar_WakeUp(cv: *mut CondVar, num_threads: i32);
}

// PTHREAD LAYER TO CALL LIBCTRU

#[no_mangle]
pub unsafe extern "C" fn pthread_create(
    _native: *mut libc::pthread_t,
    _attr: *const libc::pthread_attr_t,
    _f: extern "C" fn(_: *mut libc::c_void) -> *mut libc::c_void,
    _value: *mut libc::c_void,
) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_join(
    _native: libc::pthread_t,
    _value: *mut *mut libc::c_void,
) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_detach(_thread: libc::pthread_t) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_init(_attr: *mut libc::pthread_attr_t) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_destroy(_attr: *mut libc::pthread_attr_t) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_setstacksize(
    _attr: *mut libc::pthread_attr_t,
    _stack_size: libc::size_t,
) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn sched_yield() -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_init(
    cond: *mut libc::pthread_cond_t,
    _attr: *const libc::pthread_condattr_t,
) -> libc::c_int {
    CondVar_Init(cond as _);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_signal(cond: *mut libc::pthread_cond_t) -> libc::c_int {
    CondVar_WakeUp(cond as _, 1);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_broadcast(cond: *mut libc::pthread_cond_t) -> libc::c_int {
    CondVar_WakeUp(cond as _, -1);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_wait(
    cond: *mut libc::pthread_cond_t,
    lock: *mut libc::pthread_mutex_t,
) -> libc::c_int {
    CondVar_Wait(cond as _, lock as _);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_timedwait(
    cond: *mut libc::pthread_cond_t,
    lock: *mut libc::pthread_mutex_t,
    abstime: *const libc::timespec,
) -> libc::c_int {
    let nsec: i64 = ((*abstime).tv_sec as i64 * 1_000_000_000) + (*abstime).tv_nsec as i64;

    CondVar_WaitTimeout(cond as _, lock as _, nsec)
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cond_destroy(_cond: *mut libc::pthread_cond_t) -> libc::c_int {
    0
}

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
        LightLock_Init(lock as _);
    } else if attr == libc::PTHREAD_MUTEX_RECURSIVE {
        RecursiveLock_Init(lock as _)
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

    if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_NORMAL as u8 {
        LightLock_Lock(lock as _);
    } else if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        RecursiveLock_Lock(lock as _);
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_trylock(lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    let lock = lock as *const u8;

    if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_NORMAL as u8 {
        return LightLock_TryLock(lock as _);
    } else if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        return RecursiveLock_TryLock(lock as _);
    }

    -1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_unlock(lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    let lock = lock as *const u8;

    if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_NORMAL as u8 {
        LightLock_Unlock(lock as _);
    } else if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        RecursiveLock_Unlock(lock as _);
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutexattr_destroy(
    _attr: *mut libc::pthread_mutexattr_t,
) -> libc::c_int {
    0
}

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
            pthread_mutexattr_init(attr.as_mut_ptr());
            let mut attr = attr.assume_init();
            pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_NORMAL);

            pthread_mutex_init(&mut (*lock).mutex, &attr);
            pthread_cond_init(&mut (*lock).cvar as *mut i32 as *mut _, core::ptr::null());

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

    pthread_mutex_lock(&mut (*lock).mutex);

    while (*lock).writer_active {
        pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).num_readers += 1;

    pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_tryrdlock(
    lock: *mut libc::pthread_rwlock_t,
) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    if pthread_mutex_trylock(&mut (*lock).mutex) != 0 {
        return -1;
    }

    while (*lock).writer_active {
        pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).num_readers += 1;

    pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_wrlock(lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    pthread_mutex_lock(&mut (*lock).mutex);

    while (*lock).writer_active || (*lock).num_readers > 0 {
        pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).writer_active = true;

    pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_trywrlock(
    lock: *mut libc::pthread_rwlock_t,
) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    if pthread_mutex_trylock(&mut (*lock).mutex) != 0 {
        return -1;
    }

    while (*lock).writer_active || (*lock).num_readers > 0 {
        pthread_cond_wait(&mut (*lock).cvar as *mut i32 as _, &mut (*lock).mutex);
    }

    (*lock).writer_active = true;

    pthread_mutex_unlock(&mut (*lock).mutex);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_unlock(lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    init_rwlock(lock);
    let lock = lock as *mut rwlock_clear;

    pthread_mutex_lock(&mut (*lock).mutex);

    // If there are readers and no writer => Must be a reader
    if (*lock).num_readers > 0 && !(*lock).writer_active {
        (*lock).num_readers -= 1;

        // If there are no more readers, signal to a waiting writer
        if (*lock).num_readers == 0 {
            pthread_cond_signal(&mut (*lock).cvar as *mut i32 as _);
        }
    // If there are no readers and a writer => Must be a writer
    } else if (*lock).num_readers == 0 && (*lock).writer_active {
        (*lock).writer_active = false;

        pthread_cond_broadcast(&mut (*lock).cvar as *mut i32 as _);
    }

    pthread_mutex_unlock(&mut (*lock).mutex);

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

// THREAD KEYS IMPLEMENTATION FOR RUST STD

use spin::rwlock::RwLock;
use std::collections::BTreeMap;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

type Key = usize;
type Destructor = unsafe extern "C" fn(*mut libc::c_void);

static NEXT_KEY: AtomicUsize = AtomicUsize::new(1);

// This is a spin-lock RwLock which yields the thread every loop
static KEYS: RwLock<BTreeMap<Key, Option<Destructor>>, spin::Yield> = RwLock::new(BTreeMap::new());

#[thread_local]
static mut LOCALS: BTreeMap<Key, *mut libc::c_void> = BTreeMap::new();

fn is_valid_key(key: Key) -> bool {
    KEYS.read().contains_key(&(key as Key))
}

#[no_mangle]
pub unsafe extern "C" fn pthread_key_create(
    key: *mut libc::pthread_key_t,
    destructor: Option<Destructor>,
) -> libc::c_int {
    let new_key = NEXT_KEY.fetch_add(1, Ordering::SeqCst);
    KEYS.write().insert(new_key, destructor);

    *key = new_key as libc::pthread_key_t;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_key_delete(key: libc::pthread_key_t) -> libc::c_int {
    match KEYS.write().remove(&(key as Key)) {
        // We had a entry, so it was a valid key.
        // It's officially undefined behavior if they use the key after this,
        // so don't worry about cleaning up LOCALS, especially since we can't
        // clean up every thread's map.
        Some(_) => 0,
        // The key is unknown
        None => libc::EINVAL,
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_getspecific(key: libc::pthread_key_t) -> *mut libc::c_void {
    if let Some(&value) = LOCALS.get(&(key as Key)) {
        value as _
    } else {
        // Note: we don't care if the key is invalid, we still return null
        ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_setspecific(
    key: libc::pthread_key_t,
    value: *const libc::c_void,
) -> libc::c_int {
    let key = key as Key;

    if !is_valid_key(key) {
        return libc::EINVAL;
    }

    LOCALS.insert(key, value as *mut _);
    0
}
