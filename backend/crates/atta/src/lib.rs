//! Atta: the engine for a tree of typed files. It resolves the tree,
//! enforces its structural rules, routes each operation to the handler for
//! a node's type, and renders files.
//!
//! Types and content are opaque to it. It knows nothing about the
//! application that embeds it.
