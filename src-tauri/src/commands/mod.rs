//! Tauri command layer.
//!
//! Thin, explicitly-typed IPC entry points. Commands validate input, delegate
//! to services, and return structured `AppError`s. No business logic here
//! (tech-plan §24).
