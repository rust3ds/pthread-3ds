//! Thread keys implementation for the standard library.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{mem, ptr};

use spin::rwlock::RwLock;

type Key = libc::pthread_t;
type Destructor = unsafe extern "C" fn(*mut libc::c_void);

static NEXT_KEY: AtomicU32 = AtomicU32::new(1);

// This is a spin-lock RwLock which yields the thread every loop
static KEYS: RwLock<BTreeMap<Key, Option<Destructor>>, spin::Yield> = RwLock::new(BTreeMap::new());

#[thread_local]
static mut LOCALS: BTreeMap<Key, *mut libc::c_void> = BTreeMap::new();

fn is_valid_key(key: Key) -> bool {
    KEYS.read().contains_key(&key)
}

pub(crate) fn run_local_destructors() {
    // We iterate all the thread-local keys set.
    //
    // When using `std` and the `thread_local!` macro there should be only one key registered here,
    // which is the list of keys to destroy.
    for (key, arg) in unsafe { LOCALS.iter_mut() } {
        // We retrieve the destructor for a key from the static list. It's important
        // that we drop the KEYS write lock here to avoid deadlock that could arise
        // if the destructor itself tries to obtain a lock on KEYS.
        let maybe_dtor = {
            let mut keys = KEYS.write();
            // If the destructor is registered for a key, deregister it...
            keys.get_mut(key).and_then(Option::take)
        };

        if let Some(destructor) = maybe_dtor {
            // ... and clean up its soon-to-maybe-be-dangling arg pointer
            let arg = mem::replace(arg, ptr::null_mut());
            unsafe {
                destructor(arg);
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_key_create(
    key: *mut libc::pthread_key_t,
    destructor: Option<Destructor>,
) -> libc::c_int {
    let new_key = NEXT_KEY.fetch_add(1, Ordering::SeqCst);
    KEYS.write().insert(new_key, destructor);

    *key = new_key;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_key_delete(key: libc::pthread_key_t) -> libc::c_int {
    match KEYS.write().remove(&key) {
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
    if let Some(&value) = LOCALS.get(&key) {
        value
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
    if !is_valid_key(key) {
        return libc::EINVAL;
    }

    LOCALS.insert(key, value.cast_mut());

    0
}
