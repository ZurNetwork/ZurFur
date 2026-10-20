//! The workflow elements — the work on Zurfur.
//!
//! [`commission`] is the aggregate a commission is built from; [`workflow`]
//! holds an account's boards, where commissions are placed. Every element
//! mirrors an entry in the DESIGN glossary, which is the single source of truth
//! for what it means.

pub mod commission;
pub mod workflow;
