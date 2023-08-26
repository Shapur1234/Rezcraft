pub mod index;
pub mod loader;
pub mod pos;
#[cfg(not(target_arch = "wasm32"))]
pub mod save_helper;
mod settings;
pub mod ui;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use settings::Settings;
