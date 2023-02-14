#![feature(thread_local)]
#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

mod condvar;
mod misc;
mod mutex;
mod rwlock;
mod thread;
mod thread_keys;

/// Reference this function somewhere (eg. ´use pthread_3ds::init´ in the main crate) to import all pthread implementations.
#[inline(always)]
pub fn init() {}
