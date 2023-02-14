use static_assertions::const_assert;
use std::mem;

/// Internal struct for storing pthread attribute data
/// Must be less than or equal to the size of `libc::pthread_attr_t`. We assert
/// this below via static_assertions.
pub struct PThreadAttr {
    pub(crate) stack_size: libc::size_t,
    pub(crate) priority: libc::c_int,
    pub(crate) processor_id: libc::c_int,
}

const_assert!(mem::size_of::<PThreadAttr>() <= mem::size_of::<libc::pthread_attr_t>());

impl Default for PThreadAttr {
    fn default() -> Self {
        // Note: we are ignoring the result here, but errors shouldn't occur
        // since we're using a valid handle.
        let mut priority = 0;
        unsafe { ctru_sys::svcGetThreadPriority(&mut priority, ctru_sys::CUR_THREAD_HANDLE) };

        PThreadAttr {
            stack_size: libc::PTHREAD_STACK_MIN,

            // If no priority value is specified, spawn with the same priority
            // as the current thread
            priority,

            // If no processor is specified, spawn on the default core.
            // (determined by the application's Exheader)
            processor_id: -2,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_init(attr: *mut libc::pthread_attr_t) -> libc::c_int {
    let attr = attr as *mut PThreadAttr;
    *attr = PThreadAttr::default();

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_destroy(attr: *mut libc::pthread_attr_t) -> libc::c_int {
    std::ptr::drop_in_place(attr as *mut PThreadAttr);
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
