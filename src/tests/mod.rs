#[cfg(not(any(shuttle, loom, echeneis)))]
mod default;
mod stubs;

#[cfg(shuttle)]
mod shuttle;

#[cfg(loom)]
mod loom;
