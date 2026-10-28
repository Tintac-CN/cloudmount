//! `cmflt-core` — pure, testable logic for the CloudMount minifilter.
//!
//! The crate is `#![no_std]` and contains no I/O, no allocation, no unsafe
//! code and no kernel or platform dependency. The kernel driver and the
//! user-mode simulator execute exactly the same state machine and protocol
//! codecs; only the [`hydration::Action`] executor differs.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod coordinator;
pub mod handshake;
pub mod hydration;
pub mod journal;
pub mod placeholder;
pub mod policy;
pub mod protocol;
pub mod range;
pub mod scan;
pub mod telemetry;

#[cfg(test)]
extern crate std;
