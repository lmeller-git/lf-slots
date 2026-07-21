#![allow(unused_imports)]
#![allow(clippy::disallowed_modules)]

#[cfg(all(not(loom), not(shuttle), not(echeneis)))]
pub(crate) use core_::*;
#[cfg(all(echeneis, test))]
pub(crate) use echeneis_::*;
#[cfg(all(loom, test))]
pub(crate) use loom_::*;
#[cfg(all(shuttle, test))]
pub(crate) use shuttle_::*;

#[cfg(all(shuttle, test))]
mod shuttle_ {
    #[allow(unused_imports)]
    pub(crate) use shuttle::hint;
    pub(crate) use shuttle::{
        sync::{Arc, Condvar, Mutex, Weak, atomic},
        thread,
    };
}

#[cfg(all(loom, test))]
mod loom_ {
    // no Weak in loom
    pub(crate) use std::sync::Weak;

    pub(crate) use loom::{
        hint,
        sync::{Arc, Condvar, Mutex, atomic},
        thread,
    };
}

#[cfg(all(not(loom), not(shuttle), not(echeneis)))]
mod core_ {
    #[cfg(feature = "alloc")]
    pub(crate) use alloc::sync::{Arc, Weak};
    pub(crate) use core::hint;
    #[cfg(feature = "std")]
    pub(crate) use std::sync::{Condvar, Mutex};
    #[cfg(feature = "std")]
    pub(crate) use std::thread;

    pub(crate) use portable_atomic as atomic;
}

#[cfg(all(echeneis, test))]
mod echeneis_ {
    #[cfg(feature = "alloc")]
    pub(crate) use alloc::sync::{Arc, Weak};
    pub(crate) use core::hint;
    #[cfg(feature = "std")]
    pub(crate) use std::sync::{Condvar, Mutex};
    #[cfg(feature = "std")]
    pub(crate) use std::thread;

    pub(crate) use echeneis::sync::atomic;
}
