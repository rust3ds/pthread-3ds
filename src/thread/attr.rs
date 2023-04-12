use static_assertions::const_assert;
use std::mem;

const_assert!(mem::size_of::<ctru_sys::pthread_attr_t>() <= mem::size_of::<libc::pthread_attr_t>());

// No way to currently use any of these functions, since the thread creation syscall doesn't support them.
/*
#[no_mangle]
pub unsafe extern "C" fn pthread_attr_getprocessorid_np(
    attr: *const libc::pthread_attr_t,
    processor_id: *mut libc::c_int,
) -> libc::c_int {
    let attr = attr as *mut pthread_attr_t;
    (*processor_id) = (*attr).processor_id;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_attr_setprocessorid_np(
    attr: *mut libc::pthread_attr_t,
    processor_id: libc::c_int,
) -> libc::c_int {
    let attr = attr as *mut pthread_attr_t;
    (*attr).processor_id = processor_id;

    // TODO: we could validate the processor ID here if we wanted?

    0
}
*/