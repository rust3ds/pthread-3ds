//! PThread threads implemented using libctru.

use attr::PThreadAttr;
use spin::rwlock::RwLock;
use std::collections::BTreeMap;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod attr;

pub fn init() {}

/// The main thread's pthread ID
const MAIN_THREAD_ID: libc::pthread_t = 0;

// We initialize to 1 since 0 is reserved for the main thread.
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

// This is a spin-lock RwLock which yields the thread every loop.
static THREADS: RwLock<BTreeMap<libc::pthread_t, PThread>, spin::Yield> =
    RwLock::new(BTreeMap::new());

// Initialize to zero (main thread ID) since the main thread will have the
// default value in this thread local.
#[thread_local]
static mut THREAD_ID: libc::pthread_t = MAIN_THREAD_ID;

#[derive(Copy, Clone)]
struct PThread {
    thread: ShareablePtr<ctru_sys::Thread_tag>,
    os_thread_id: u32,
    is_detached: bool,
    is_finished: bool,
    result: ShareablePtr<libc::c_void>,
}

/// Pointers are not Send or Sync, though it's really just a lint. This struct
/// lets us ignore that "lint".
struct ShareablePtr<T>(*mut T);
unsafe impl<T> Send for ShareablePtr<T> {}
unsafe impl<T> Sync for ShareablePtr<T> {}

// We can't use the derives because they add an unnecessary T: Copy/Clone bound.
impl<T> Copy for ShareablePtr<T> {}
impl<T> Clone for ShareablePtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_create(
    native: *mut libc::pthread_t,
    attr: *const libc::pthread_attr_t,
    entrypoint: extern "C" fn(_: *mut libc::c_void) -> *mut libc::c_void,
    value: *mut libc::c_void,
) -> libc::c_int {
    let attr = attr as *const PThreadAttr;
    let stack_size = (*attr).stack_size;
    let priority = (*attr).priority;
    let processor_id = (*attr).processor_id;

    let thread_id = NEXT_ID.fetch_add(1, Ordering::SeqCst) as libc::pthread_t;

    extern "C" fn thread_start(main: *mut libc::c_void) {
        unsafe {
            Box::from_raw(main as *mut Box<dyn FnOnce()>)();
        }
    }

    // The closure needs a fat pointer (64 bits) to work since it captures a variable and is thus a
    // trait object, but *mut void is only 32 bits. We need double indirection to pass along the
    // full closure data.
    // We make this closure in the first place because threadCreate expects a void return type, but
    // entrypoint returns a pointer so the types are incompatible.
    let main: *mut Box<dyn FnOnce()> = Box::into_raw(Box::new(Box::new(move || {
        THREAD_ID = thread_id;

        let result = entrypoint(value);

        // Update the threads map with the result, and remove this thread if
        // it's detached.
        // We hold the lock the whole time so there isn't a race condition.
        // (ex. we copy out the thread data, pthread_detach is called, we
        // check is_detached and it's still false, so thread is never
        // removed from the map)
        let mut thread_map = THREADS.write();
        if let Some(mut pthread) = thread_map.get_mut(&thread_id) {
            pthread.is_finished = true;
            pthread.result.0 = result;

            if pthread.is_detached {
                // libctru will call threadFree once this thread dies
                thread_map.remove(&thread_id);
            }
        }
    })));

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

    // Get the OS thread ID
    let os_handle = ctru_sys::threadGetHandle(thread);
    let mut os_thread_id = 0;
    let result = ctru_sys::svcGetThreadId(&mut os_thread_id, os_handle);
    if ctru_sys::R_FAILED(result) {
        // TODO: improve error handling? Different codes?
        return libc::EPERM;
    }

    // Insert the thread into our map
    THREADS.write().insert(
        thread_id,
        PThread {
            thread: ShareablePtr(thread),
            os_thread_id,
            is_detached: false,
            is_finished: false,
            result: ShareablePtr(ptr::null_mut()),
        },
    );

    *native = thread_id;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_join(
    thread_id: libc::pthread_t,
    return_value: *mut *mut libc::c_void,
) -> libc::c_int {
    if thread_id == MAIN_THREAD_ID {
        // This is not a valid thread to join on
        return libc::EINVAL;
    }

    let thread_map = THREADS.read();
    let Some(&pthread) = thread_map.get(&thread_id) else {
        // This is not a valid thread ID
        return libc::ESRCH
    };
    // We need to drop our read guard so it doesn't stay locked while joining
    // the thread.
    drop(thread_map);

    if pthread.is_detached {
        // Cannot join on a detached thread
        return libc::EINVAL;
    }

    let result = ctru_sys::threadJoin(pthread.thread.0, u64::MAX);
    if ctru_sys::R_FAILED(result) {
        // TODO: improve the error code by checking the result further?
        return libc::EINVAL;
    }

    ctru_sys::threadFree(pthread.thread.0);
    let thread_data = THREADS.write().remove(&thread_id);

    // This should always be Some, but we use an if let just in case.
    if let Some(thread_data) = thread_data {
        if !return_value.is_null() {
            *return_value = thread_data.result.0;
        }
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_detach(thread_id: libc::pthread_t) -> libc::c_int {
    if thread_id == MAIN_THREAD_ID {
        // This is not a valid thread to detach
        return libc::EINVAL;
    }

    let mut thread_map = THREADS.write();
    let Some(mut pthread) = thread_map.get_mut(&thread_id) else {
        // This is not a valid thread ID
        return libc::ESRCH
    };

    if pthread.is_detached {
        // Cannot detach an already detached thread
        return libc::EINVAL;
    }

    pthread.is_detached = true;
    ctru_sys::threadDetach(pthread.thread.0);

    if pthread.is_finished {
        // threadFree was already called by threadDetach
        thread_map.remove(&thread_id);
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_self() -> libc::pthread_t {
    let thread_id = THREAD_ID;

    if thread_id == MAIN_THREAD_ID {
        // Take this opportunity to populate the main thread's data in the map.
        // They can only "legally" get the main thread's ID by calling this
        // function, so this will run before they do anything that needs this.

        // We have to ignore the possible error from svcGetThreadId since
        // pthread_self cannot fail... But there shouldn't be an error anyways.
        let mut os_thread_id = 0;
        let _ = ctru_sys::svcGetThreadId(&mut os_thread_id, ctru_sys::CUR_THREAD_HANDLE);

        THREADS.write().insert(
            MAIN_THREAD_ID,
            PThread {
                // This null pointer is safe because we return before ever using
                // it (in pthread_join and pthread_detach).
                thread: ShareablePtr(ptr::null_mut()),
                os_thread_id,
                is_detached: true,
                is_finished: false,
                result: ShareablePtr(ptr::null_mut()),
            },
        );
    }

    thread_id
}

/// Closes a kernel handle on drop.
struct Handle(ctru_sys::Handle);

impl TryFrom<libc::pthread_t> for Handle {
    type Error = libc::c_int;

    fn try_from(thread_id: libc::pthread_t) -> Result<Self, Self::Error> {
        let Some(&pthread) = THREADS.read().get(&thread_id) else {
            // This is not a valid thread ID
            return Err(libc::ESRCH)
        };

        if pthread.is_finished {
            return Err(libc::ESRCH);
        }

        let mut os_handle = 0;
        let ret = unsafe {
            ctru_sys::svcOpenThread(
                &mut os_handle,
                ctru_sys::CUR_PROCESS_HANDLE,
                pthread.os_thread_id,
            )
        };

        if ctru_sys::R_FAILED(ret) || os_handle == u32::MAX {
            // Either there was an error or the thread already finished
            Err(libc::ESRCH)
        } else {
            Ok(Self(os_handle))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            // We ignore the error because we can't return it or panic.
            let _ = ctru_sys::svcCloseHandle(self.0);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_getschedparam(
    thread_id: libc::pthread_t,
    policy: *mut libc::c_int,
    param: *mut libc::sched_param,
) -> libc::c_int {
    let handle = match Handle::try_from(thread_id) {
        Ok(handle) => handle,
        Err(code) => return code,
    };

    let mut priority = 0;
    let result = ctru_sys::svcGetThreadPriority(&mut priority, handle.0);
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
    thread_id: libc::pthread_t,
    policy: libc::c_int,
    param: *const libc::sched_param,
) -> libc::c_int {
    if policy != libc::SCHED_FIFO {
        // We only accept SCHED_FIFO. See the note in pthread_getschedparam.
        return libc::EINVAL;
    }

    let handle = match Handle::try_from(thread_id) {
        Ok(handle) => handle,
        Err(code) => return code,
    };

    let result = ctru_sys::svcSetThreadPriority(handle.0, (*param).sched_priority);
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
