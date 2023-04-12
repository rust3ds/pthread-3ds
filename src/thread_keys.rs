//! Thread keys implementation for the standard library.

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
pub unsafe extern "C" fn  __syscall_tls_create(
    key: *mut libc::pthread_key_t,
    destructor: Option<Destructor>,
) -> libc::c_int {
    let new_key = NEXT_KEY.fetch_add(1, Ordering::SeqCst);
    KEYS.write().insert(new_key, destructor);

    *key = new_key as libc::pthread_key_t;

    0
}

#[no_mangle]
pub unsafe extern "C" fn  __syscall_tls_delete(key: libc::pthread_key_t) -> libc::c_int {
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
pub unsafe extern "C" fn  __syscall_tls_get(key: libc::pthread_key_t) -> *mut libc::c_void {
    if let Some(&value) = LOCALS.get(&(key as Key)) {
        value as _
    } else {
        // Note: we don't care if the key is invalid, we still return null
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn  __syscall_tls_set(
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
