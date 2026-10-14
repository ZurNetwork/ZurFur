//! The Den read: a viewer's own root and the nodes below it, resolved by Atta through
//! Zurfur's one host. Atta's types stay inside this module; its use cases take and answer
//! the application's own values.

pub(crate) mod access;
mod admission;
mod context;
mod den_type;
pub(crate) mod host;
mod kind;
mod level;
mod name;
pub(crate) mod names;
pub mod node;
mod output;
mod page_token;
mod place;
pub(crate) mod post;
pub(crate) mod read;
pub mod root;
mod segment;
mod soft_delete;
mod state;

use crate::ports::WithPorts;

pub use den_type::DenType;
pub use kind::NodeKind;
pub use level::NodeLevel;
pub use name::{DenName, DenNameError};
pub use output::{DenAccess, DenContent, DenCrumb, DenListing, DenNode};
pub use page_token::{PageToken, PageTokenError};
pub use segment::DenSegment;
pub use soft_delete::SoftDelete;

pub(crate) use atta::Level;
pub(crate) use atta::NodeName;
pub(crate) use atta::Segment;
pub(crate) use context::{DenContext, RootNaming};
pub(crate) use place::STANDING;
pub(crate) use place::{DenMount, Node, Place, TopLevel};
pub(crate) use state::NodeState;

/// Den use cases, with the ports already bound.
#[derive(Clone, Copy, WithPorts)]
pub struct Den<'a> {
    #[ports(is_root = true)]
    ports: &'a crate::Ports,
}

impl<'a> From<&'a crate::Ports> for Den<'a> {
    /// Binds the namespace to the bag.
    fn from(ports: &'a crate::Ports) -> Self {
        Self { ports }
    }
}

impl<'a> From<&'a crate::App> for Den<'a> {
    /// Binds the namespace to the app's ports.
    fn from(app: &'a crate::App) -> Self {
        Self::from(app.ports())
    }
}

/// Why a Den read could not answer. Every miss, whatever its cause, is the one
/// [`DenError::NotFound`]; `Display` never interpolates the cause, which rides `source()`.
#[derive(Debug, thiserror::Error)]
pub enum DenError {
    /// The node is absent, doesn't exist, sits inside one the viewer can't open, or the path
    /// is malformed.
    #[error("node not found")]
    NotFound,
    /// A store behind the Den failed, or the host broke one of its own rules.
    #[error("the den could not be read")]
    Store(#[source] anyhow::Error),
}
