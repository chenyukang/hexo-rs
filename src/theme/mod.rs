//! Theme module - handles theme loading
//!
//! Note: EJS/QuickJS has been removed. We now use Tera templates
//! with the vexo theme embedded directly in the binary.

mod loader;

pub use loader::ThemeLoader;
