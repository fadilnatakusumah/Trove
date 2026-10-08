//! Application services.
//!
//! `skill_service`, `project_service`, `install_service`, and
//! `filesystem_service` live here. Services contain all domain logic;
//! commands and UI layers stay thin (tech-plan §8).

pub mod filesystem_service;
pub mod git_service;
