//! Integration-only substrate modules.
//!
//! These modules consume the platform substrate to answer readiness,
//! coverage, evidence, and release-process questions. They are deliberately
//! separated from the platform substrate modules because they may describe
//! downstream integration surfaces while the platform itself remains
//! consumer-neutral.

pub mod coverage;
pub mod evidence;
pub mod quality;
pub mod release;
