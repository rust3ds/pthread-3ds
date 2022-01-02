#![feature(thread_local)]
#![allow(non_camel_case_types)]

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

pub type LightLock = _LOCK_T;
pub type RecursiveLock = _LOCK_RECURSIVE_T;
pub type CondVar = i32;
pub type Thread = *mut libc::c_void;
pub type ThreadFunc = extern "C" fn(arg1: *mut libc::c_void);

type ResultCode = u32;

extern "C" {
    pub fn svcSleepThread(ns: i64);
    pub fn svcGetThreadPriority(out: *mut i32, handle: u32) -> ResultCode;

    pub fn threadCreate(
        entrypoint: ThreadFunc,
        arg: *mut libc::c_void,
        stack_size: libc::size_t,
        prio: libc::c_int,
        core_id: libc::c_int,
        detached: bool,
    ) -> Thread;
    pub fn threadJoin(thread: Thread, timeout_ns: u64) -> ResultCode;
    pub fn threadFree(thread: Thread);
    pub fn threadDetach(thread: Thread);

    pub fn LightLock_Init(lock: *mut LightLock);
    pub fn LightLock_Lock(lock: *mut LightLock);
    pub fn LightLock_TryLock(lock: *mut LightLock) -> libc::c_int;
    pub fn LightLock_Unlock(lock: *mut LightLock);

    pub fn RecursiveLock_Init(lock: *mut RecursiveLock);
    pub fn RecursiveLock_Lock(lock: *mut RecursiveLock);
    pub fn RecursiveLock_TryLock(lock: *mut RecursiveLock) -> libc::c_int;
    pub fn RecursiveLock_Unlock(lock: *mut RecursiveLock);

    pub fn CondVar_Init(cv: *mut CondVar);
    pub fn CondVar_Wait(cv: *mut CondVar, lock: *mut LightLock);
    pub fn CondVar_WaitTimeout(
        cv: *mut CondVar,
        lock: *mut LightLock,
        timeout_ns: i64,
    ) -> libc::c_int;
    pub fn CondVar_WakeUp(cv: *mut CondVar, num_threads: i32);
}

// PTHREAD LAYER TO CALL LIBCTRU

#[no_mangle]
pub unsafe extern "C" fn pthread_create(
    native: *mut libc::pthread_t,
    attr: *const libc::pthread_attr_t,
    _f: extern "C" fn(_: *mut libc::c_void) -> *mut libc::c_void,
    value: *mut libc::c_void,
) -> libc::c_int {
    let mut priority = 0;
    svcGetThreadPriority(&mut priority, 0xFFFF8000);

    extern "C" fn thread_start(main: *mut libc::c_void) {
        unsafe {
            Box::from_raw(main as *mut Box<dyn FnOnce()>)();
        }
    }

    let handle = threadCreate(
        thread_start,
        value,
        *(attr as *mut libc::size_t),
        priority,
        -2,
        false,
    );

    *native = handle as _;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_join(
    native: libc::pthread_t,
    _value: *mut *mut libc::c_void,
) -> libc::c_int {
    threadJoin(native as *mut libc::c_void, u64::max_value());
    threadFree(native as *mut libc::c_void);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_detach(thread: libc::pthread_t) -> libc::c_int {
    threadDetach(thread as _);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_init(_attr: *mut libc::pthread_attr_t) -> libc::c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_destroy(_attr: *mut libc::pthread_attr_t) -> libc::c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_setstacksize(
    attr: *mut libc::pthread_attr_t,
    stack_size: libc::size_t,
) -> libc::c_int {
    let pointer = attr as *mut libc::size_t;
    *pointer = stack_size;

    0
}

#[no_mangle]
pub unsafe extern "C" fn sched_yield() -> libc::c_int {
    svcSleepThread(0);

    0
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

    0
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

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_init(
    _lock: *mut libc::pthread_rwlock_t,
    _attr: *const libc::pthread_rwlockattr_t,
) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_destroy(_lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_rdlock(_lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_tryrdlock(
    _lock: *mut libc::pthread_rwlock_t,
) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_wrlock(_lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_trywrlock(
    _lock: *mut libc::pthread_rwlock_t,
) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlock_unlock(_lock: *mut libc::pthread_rwlock_t) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlockattr_init(
    _attr: *mut libc::pthread_rwlockattr_t,
) -> libc::c_int {
    1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_rwlockattr_destroy(
    _attr: *mut libc::pthread_rwlockattr_t,
) -> libc::c_int {
    1
}

// THREAD KEYS IMPLEMENTATION FOR RUST STD

use std::collections::BTreeMap;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type Key = usize;

type Dtor = unsafe extern "C" fn(*mut u8);

static NEXT_KEY: AtomicUsize = AtomicUsize::new(1);

static mut KEYS: *mut BTreeMap<Key, Option<Dtor>> = ptr::null_mut();

#[thread_local]
static mut LOCALS: *mut BTreeMap<Key, *mut u8> = ptr::null_mut();

unsafe fn keys() -> &'static mut BTreeMap<Key, Option<Dtor>> {
    if KEYS == ptr::null_mut() {
        KEYS = Box::into_raw(Box::new(BTreeMap::new()));
    }
    &mut *KEYS
}

unsafe fn locals() -> &'static mut BTreeMap<Key, *mut u8> {
    if LOCALS == ptr::null_mut() {
        LOCALS = Box::into_raw(Box::new(BTreeMap::new()));
    }
    &mut *LOCALS
}

#[inline]
pub unsafe fn create(dtor: Option<unsafe extern "C" fn(*mut u8)>) -> Key {
    let key = NEXT_KEY.fetch_add(1, Ordering::SeqCst);
    keys().insert(key, dtor);
    key
}

#[no_mangle]
pub unsafe extern "C" fn pthread_key_create(
    key: *mut libc::pthread_key_t,
    dtor: Option<unsafe extern "C" fn(*mut libc::c_void)>,
) -> libc::c_int {
    let new_key = NEXT_KEY.fetch_add(1, Ordering::SeqCst);
    keys().insert(new_key, std::mem::transmute(dtor));

    *key = new_key as u32;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_key_delete(key: libc::pthread_key_t) -> libc::c_int {
    keys().remove(&(key as usize));

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_getspecific(key: libc::pthread_key_t) -> *mut libc::c_void {
    if let Some(&entry) = locals().get(&(key as usize)) {
        entry as _
    } else {
        ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_setspecific(
    key: libc::pthread_key_t,
    value: *const libc::c_void,
) -> libc::c_int {
    locals().insert(key as usize, std::mem::transmute(value));
    0
}
