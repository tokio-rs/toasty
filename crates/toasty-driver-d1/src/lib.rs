#![warn(missing_docs)]

//! Toasty driver for [Cloudflare D1](https://developers.cloudflare.com/d1/).
//!
//! D1 drivers are constructed from a request-local Worker binding and use a
//! direct connection rather than a connection pool.

mod migration;

#[cfg(target_arch = "wasm32")]
mod connection;
#[cfg(target_arch = "wasm32")]
mod driver;
#[cfg(target_arch = "wasm32")]
mod error;
#[cfg(any(target_arch = "wasm32", test))]
mod value;

pub use migration::generate_migration;

#[cfg(target_arch = "wasm32")]
pub use driver::D1;
