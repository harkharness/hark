//! vox-core: the functional core and ports of Vox.
//!
//! Layout (hexagonal / functional core, imperative shell):
//! - `domain`: pure functions and immutable types. No I/O, fully unit-testable.
//! - `ports`: traits that the outside world implements.
//! - `adapters`: thin imperative implementations of the ports.
//! - `app`: use cases wiring domain + ports together.

pub mod app;
pub mod adapters;
pub mod domain;
pub mod ports;
