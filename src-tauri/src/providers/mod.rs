//! Agent provider layer.
//!
//! One module per supported agent (claude, cursor, codex). Each provider owns
//! only its own filesystem conventions; providers are assembled behind the
//! `AgentProvider` trait in plan 04 (tech-plan §10 — no abstraction before two
//! real implementations exist).
