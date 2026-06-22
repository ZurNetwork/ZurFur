//! Workflows (a.k.a. Views) — ways to organize commissions. **Stub: empty.**
//!
//! A Workflow organizes [`crate::elements::commission::Commission`]s by layout
//! or algorithm (DESIGN/Workflow). Under the Plugin-First Architecture a Workflow
//! knows about commissions, never the reverse; presentation lives in the
//! frontend, composition in the backend. Underneath, every workflow is the same
//! primitive: an ordered selection over commissions. Destined for the `workflow`
//! per-domain crate once built out — nothing is modelled here yet.
