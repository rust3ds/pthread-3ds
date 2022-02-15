#![feature(thread_local)]
#![feature(const_btree_new)]
#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use std::ptr;

/// Call this somewhere to force Rust to link this module.
/// The call doesn't need to execute, just exist.
///
/// See https://github.com/rust-lang/rust/issues/47384
pub fn init() {}

// PTHREAD LAYER TO CALL LIBCTRU

/// The main thread's thread ID. It is "null" because libctru didn't spawn it.
const MAIN_THREAD_ID: libc::pthread_t = 0;

#[no_mangle]
pub unsafe extern "C" fn pthread_create(
    native: *mut libc::pthread_t,
    attr: *const libc::pthread_attr_t,
    entrypoint: extern "C" fn(_: *mut libc::c_void) -> *mut libc::c_void,
    value: *mut libc::c_void,
) -> libc::c_int {
    let attr = attr as *const PThreadAttr;
    let stack_size = (*attr).stack_size as ctru_sys::size_t;
    let priority = (*attr).priority;
    let processor_id = (*attr).processor_id;

    extern "C" fn thread_start(main: *mut libc::c_void) {
        unsafe {
            Box::from_raw(main as *mut Box<dyn FnOnce() -> *mut libc::c_void>)();
        }
    }

    // The closure needs a fat pointer (64 bits) to work since it captures a variable and is thus a
    // trait object, but *mut void is only 32 bits. We need double indirection to pass along the
    // full closure data.
    // We make this closure in the first place because threadCreate expects a void return type, but
    // entrypoint returns a pointer so the types are incompatible.
    let main: *mut Box<dyn FnOnce() -> *mut libc::c_void> =
        Box::into_raw(Box::new(Box::new(move || entrypoint(value))));

    let thread = ctru_sys::threadCreate(
        Some(thread_start),
        main as *mut libc::c_void,
        stack_size,
        priority,
        processor_id,
        false,
    );

    if thread.is_null() {
        // There was some error, but libctru doesn't expose the result.
        // We assume there was permissions issue (such as too low of a priority).
        // We also need to clean up the closure at this time.
        drop(Box::from_raw(main));
        return libc::EPERM;
    }

    *native = thread as _;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_join(
    native: libc::pthread_t,
    _value: *mut *mut libc::c_void,
) -> libc::c_int {
    if native == MAIN_THREAD_ID {
        // This is not a valid thread to join on
        return libc::EINVAL;
    }

    let result = ctru_sys::threadJoin(native as ctru_sys::Thread, u64::MAX);
    if ctru_sys::R_FAILED(result) {
        // TODO: improve the error code by checking the result further?
        return libc::EINVAL;
    }

    ctru_sys::threadFree(native as ctru_sys::Thread);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_detach(thread: libc::pthread_t) -> libc::c_int {
    if thread == MAIN_THREAD_ID {
        // This is not a valid thread to detach
        return libc::EINVAL;
    }

    ctru_sys::threadDetach(thread as ctru_sys::Thread);

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_self() -> libc::pthread_t {
    ctru_sys::threadGetCurrent() as libc::pthread_t
}

unsafe fn get_main_thread_handle() -> Result<ctru_sys::Handle, ctru_sys::Result> {
    // Unfortunately I don't know of any better way to get the main thread's
    // handle, other than via svcGetThreadList and svcOpenThread.
    // Experimentally, the main thread ID is always the first in the list.
    let mut thread_ids = [0; 1];
    let mut thread_ids_count = 0;
    let result = ctru_sys::svcGetThreadList(
        &mut thread_ids_count,
        thread_ids.as_mut_ptr(),
        thread_ids.len() as i32,
        ctru_sys::CUR_PROCESS_HANDLE,
    );
    if ctru_sys::R_FAILED(result) {
        return Err(result);
    }

    // Get the main thread's handle
    let main_thread_id = thread_ids[0];
    let mut main_thread_handle = 0;
    let result = ctru_sys::svcOpenThread(
        &mut main_thread_handle,
        ctru_sys::CUR_PROCESS_HANDLE,
        main_thread_id,
    );
    if ctru_sys::R_FAILED(result) {
        return Err(result);
    }

    Ok(main_thread_handle)
}

unsafe fn get_thread_handle(native: libc::pthread_t) -> Result<ctru_sys::Handle, ctru_sys::Result> {
    if native == MAIN_THREAD_ID {
        get_main_thread_handle()
    } else {
        Ok(ctru_sys::threadGetHandle(native as ctru_sys::Thread))
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_getschedparam(
    native: libc::pthread_t,
    policy: *mut libc::c_int,
    param: *mut libc::sched_param,
) -> libc::c_int {
    let handle = match get_thread_handle(native) {
        Ok(handle) => handle,
        Err(_) => return libc::ESRCH,
    };

    if handle == u32::MAX {
        // The thread has already finished
        return libc::ESRCH;
    }

    let mut priority = 0;
    let result = ctru_sys::svcGetThreadPriority(&mut priority, handle);
    if ctru_sys::R_FAILED(result) {
        // Some error occurred. This is the only error defined for this
        // function, so return it. Maybe the thread exited while this function
        // was exiting?
        return libc::ESRCH;
    }

    (*param).sched_priority = priority;

    // SCHED_FIFO is closest to how the cooperative app core works, while
    // SCHED_RR is closest to how the preemptive sys core works.
    // However, we don't have an API to get the current processor ID of a chosen
    // thread (only the current), so we just always return SCHED_FIFO.
    (*policy) = libc::SCHED_FIFO;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_setschedparam(
    native: libc::pthread_t,
    policy: libc::c_int,
    param: *const libc::sched_param,
) -> libc::c_int {
    if policy != libc::SCHED_FIFO {
        // We only accept SCHED_FIFO. See the note in pthread_getschedparam.
        return libc::EINVAL;
    }

    let handle = match get_thread_handle(native) {
        Ok(handle) => handle,
        Err(_) => return libc::EINVAL,
    };

    if handle == u32::MAX {
        // The thread has already finished
        return libc::ESRCH;
    }

    let result = ctru_sys::svcSetThreadPriority(handle, (*param).sched_priority);
    if ctru_sys::R_FAILED(result) {
        // Probably the priority is out of the permissible bounds
        // TODO: improve the error code by checking the result further?
        return libc::EPERM;
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_getprocessorid_np() -> libc::c_int {
    ctru_sys::svcGetProcessorID()
}

/// Internal struct for storing pthread attribute data
/// Must be less than or equal to the size of `libc::pthread_attr_t`. We assert
/// this below via static_assertions.
struct PThreadAttr {
    stack_size: libc::size_t,
    priority: libc::c_int,
    processor_id: libc::c_int,
}

impl Default for PThreadAttr {
    fn default() -> Self {
        let thread_id = unsafe { libc::pthread_self() };
        let mut policy = 0;
        let mut sched_param = libc::sched_param { sched_priority: 0 };

        unsafe { libc::pthread_getschedparam(thread_id, &mut policy, &mut sched_param) };

        let priority = sched_param.sched_priority;

        PThreadAttr {
            stack_size: libc::PTHREAD_STACK_MIN,

            // If no priority value is specified, spawn with the same
            // priority as the current thread
            priority,

            // If no processor is specified, spawn on the default core.
            // (determined by the application's Exheader)
            processor_id: -2,
        }
    }
}

static_assertions::const_assert!(
    std::mem::size_of::<PThreadAttr>() <= std::mem::size_of::<libc::pthread_attr_t>()
);

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_init(attr: *mut libc::pthread_attr_t) -> libc::c_int {
    let attr = attr as *mut PThreadAttr;
    *attr = PThreadAttr::default();

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_destroy(attr: *mut libc::pthread_attr_t) -> libc::c_int {
    ptr::drop_in_place(attr as *mut PThreadAttr);
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_setstacksize(
    attr: *mut libc::pthread_attr_t,
    stack_size: libc::size_t,
) -> libc::c_int {
    let attr = attr as *mut PThreadAttr;
    (*attr).stack_size = stack_size;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_getschedparam(
    attr: *const libc::pthread_attr_t,
    param: *mut libc::sched_param,
) -> libc::c_int {
    let attr = attr as *const PThreadAttr;
    (*param).sched_priority = (*attr).priority;
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_setschedparam(
    attr: *mut libc::pthread_attr_t,
    param: *const libc::sched_param,
) -> libc::c_int {
    let attr = attr as *mut PThreadAttr;
    (*attr).priority = (*param).sched_priority;

    // TODO: we could validate the priority here if we wanted?

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_getprocessorid_np(
    attr: *const libc::pthread_attr_t,
    processor_id: *mut libc::c_int,
) -> libc::c_int {
    let attr = attr as *mut PThreadAttr;
    (*processor_id) = (*attr).processor_id;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_setprocessorid_np(
    attr: *mut libc::pthread_attr_t,
    processor_id: libc::c_int,
) -> libc::c_int {
    let attr = attr as *mut PThreadAttr;
    (*attr).processor_id = processor_id;

    // TODO: we could validate the processor ID here if we wanted?

    0
}

#[no_mangle]
pub unsafe extern "C" fn sched_yield() -> libc::c_int {
    ctru_sys::svcSleepThread(0);

    0
}

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
    let r = libc::gettimeofday(&mut now, ptr::null_mut());
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

    if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_NORMAL as u8 {
        ctru_sys::LightLock_Lock(lock as _);
    } else if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        ctru_sys::RecursiveLock_Lock(lock as _);
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_trylock(lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    let lock = lock as *const u8;

    if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_NORMAL as u8 {
        return ctru_sys::LightLock_TryLock(lock as _);
    } else if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
        return ctru_sys::RecursiveLock_TryLock(lock as _);
    }

    -1
}

#[no_mangle]
pub unsafe extern "C" fn pthread_mutex_unlock(lock: *mut libc::pthread_mutex_t) -> libc::c_int {
    let lock = lock as *const u8;

    if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_NORMAL as u8 {
        ctru_sys::LightLock_Unlock(lock as _);
    } else if *(lock.offset(39)) as u8 == libc::PTHREAD_MUTEX_RECURSIVE as u8 {
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

#[no_mangle]
pub unsafe extern "C" fn pthread_sigmask(
    _how: ::libc::c_int,
    _set: *const libc::sigset_t,
    _oldset: *mut libc::sigset_t,
) -> ::libc::c_int {
    -1
}

// THREAD KEYS IMPLEMENTATION FOR RUST STD

use spin::rwlock::RwLock;
use std::collections::BTreeMap;
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
