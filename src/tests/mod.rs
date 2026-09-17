#[cfg(not(any(shuttle, loom)))]
mod default;
mod stubs;

#[cfg(shuttle)]
mod shuttle;

#[cfg(loom)]
mod loom;
