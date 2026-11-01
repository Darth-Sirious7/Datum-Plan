//! Datum Plan core: everything except the window. No UI or OS code lives here.
pub mod cal;
pub mod check;
pub mod doc;
pub mod fields;
pub mod formula;
pub mod io;
pub mod level;
pub mod links;
pub mod load;
pub mod model;
pub mod network;
pub mod ops;
pub mod parse;
pub mod progress;
pub mod query;
pub mod report;
pub mod risk;
pub mod sched;
pub mod sheet;
pub mod templates;
pub mod tutorial;

pub use model::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod hardening;
#[cfg(test)]
mod cases;
#[cfg(test)]
mod exchange_cases;
#[cfg(test)]
mod sched_cases;
#[cfg(test)]
mod formula_cases;
#[cfg(test)]
mod parity;
