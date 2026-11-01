//! Operating-system integration. Nexdir currently targets Windows only.

#[cfg(not(windows))]
compile_error!("Nexdir currently supports Windows only.");

#[cfg(windows)]
pub mod clipboard;
#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use self::win32::*;
