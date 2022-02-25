//! PThread threads implemented using libctru.

use attr::PThreadAttr;

mod attr;

pub fn init() {
    attr::init();
}

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
