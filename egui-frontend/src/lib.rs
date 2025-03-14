#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::App;

pub mod fits;
pub mod indi;
pub mod task;
