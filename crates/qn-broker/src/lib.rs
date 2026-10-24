//! Host-side credential broker adapter for qn-sandbox.
//! Grants contain routing references only; providers and authentication are separate.
mod grants;
pub use grants::{CaptureScope, GrantError, GrantRegistry};
