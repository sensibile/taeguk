pub mod model;
mod store;
#[path = "custom/extra.rs"]
pub mod extra;
#[cfg(feature = "absent")]
mod missing;

pub use store::Store;
#[cfg(unix)]
#[path = "sys/unix.rs"]
mod imp;
#[cfg(windows)]
#[path = "sys/windows.rs"]
mod imp;

pub fn platform() -> &'static str {
    imp::NAME
}
pub mod implementation;
pub use crate::implementation as facade;
pub use fx_base::imp as based;
