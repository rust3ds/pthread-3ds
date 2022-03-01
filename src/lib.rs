#![feature(thread_local)]
#![feature(const_btree_new)]
#![feature(let_else)]
#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

mod condvar;
mod misc;
mod mutex;
mod rwlock;
mod thread;
mod thread_keys;

/// Call this somewhere to force Rust to link this module.
/// The call doesn't need to execute, just exist.
///
/// See https://github.com/rust-lang/rust/issues/47384
pub fn init() {
    condvar::init();
    misc::init();
    mutex::init();
    rwlock::init();
    thread::init();
    thread_keys::init();
}
