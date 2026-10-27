//! Minimal, hand-written FltMgr binding layer.
//!
//! There is deliberately no `bindgen`, no `libclang` and no C header in this
//! crate. The ABI is derived from the documented WDK declarations and checked
//! with compile-time size assertions. Only the symbols required by the
//! current milestone are declared; unused FltMgr surface is not exposed.
//!
//! The `raw` module is the only place that may be unsound; `safe` wraps the
//! lifetime of a registered filter and turns status codes into `Result`.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod raw;

pub mod safe;

pub use safe::{nt_success, unregister_raw, FilterRegistration, RegisteredFilter};

#[cfg(test)]
extern crate std;
