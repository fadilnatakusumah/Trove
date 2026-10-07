//! Structured error type (`AppError`).
//!
//! Every command returns `Result<T, AppError>`; errors serialize to the
//! frontend as `{ code, message }` objects (tech-plan §25). Human-readable
//! messages only — never raw OS errors.
