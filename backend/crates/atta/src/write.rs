use std::{collections::HashSet, fmt};

use crate::{Act, Born, Host, HostError, Kind, Level, Placed, Shape, Type, Viewer};

/// Why Atta refused an act before any host saw it. Carries no part of the act.
#[derive(Clone, Copy, PartialEq, Eq, Hash, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum Refusal {
    /// A node would be made inside a file, or a born file would hold entries.
    #[error("a file holds no entries")]
    EntriesInFile,
    /// A born node would be wider than its parent, or an entry wider than its node.
    #[error("a node is never wider than its parent")]
    WiderThanParent,
    /// A born node would lie deeper than the depth cap.
    #[error("a node would lie deeper than the depth cap")]
    TooDeep,
    /// The act would act on, or make, a node of another type than its own.
    #[error("an act acts only on a node of its own type")]
    OtherType,
    /// Two entries of one act would share a key.
    #[error("two entries share one key")]
    DuplicateKey,
}

/// Why a write failed.
#[derive(thiserror::Error)]
pub enum WriteError {
    /// Atta refused the act; no host saw it.
    #[error(transparent)]
    Refused(#[from] Refusal),
    /// The host failed, or refused to apply the act.
    #[error(transparent)]
    Host(#[from] HostError),
}

/// An act that passed Atta's generic checks, with each born node's settled level. Only
/// [`Writer::act`] makes one. It proves that the checks ran, never that the viewer may act.
pub struct Checked<B> {
    act: Act<B>,
    settled: Vec<Level>,
}

impl<B> Checked<B> {
    /// The act, exactly as it was given.
    pub fn act(&self) -> &Act<B> {
        &self.act
    }

    /// Each node the act makes, the node first and then its entries, with its settled level.
    /// Empty for an act that makes nothing.
    pub fn born(&self) -> impl Iterator<Item = (&Born, Level)> {
        let born: Vec<&Born> = match &self.act.shape {
            Shape::Create { node, entries, .. } => std::iter::once(node).chain(entries).collect(),
            Shape::Rename { .. } | Shape::Change { .. } => Vec::new(),
        };
        born.into_iter().zip(self.settled.iter().copied())
    }

    /// The act, for the host to apply. A method, not `From`: it takes the proof apart, the
    /// owned twin of [`Checked::act`], as std's `into_inner` methods do.
    pub fn into_act(self) -> Act<B> {
        self.act
    }
}

/// The write caps the host sets for one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteCaps {
    /// The deepest a written node may lie, in real segments below `/`.
    pub max_real_depth: usize,
}

impl Default for WriteCaps {
    /// Writes at most 8 real segments deep.
    fn default() -> Self {
        Self { max_real_depth: 8 }
    }
}

/// Writes the tree for one viewer: checks each act, then hands it to the host.
pub struct Writer {
    /// Who is acting; Atta passes it to the host and never inspects it.
    pub viewer: Viewer,
    /// The write caps.
    pub caps: WriteCaps,
}

/// How an act ended, by the name the writer's debug event logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
enum Outcome {
    /// The host applied it.
    Applied,
    /// Atta refused it.
    Refused,
    /// The host failed or refused it.
    HostError,
}

impl<P> From<&Result<P, WriteError>> for Outcome {
    fn from(written: &Result<P, WriteError>) -> Self {
        match written {
            Ok(_) => Self::Applied,
            Err(WriteError::Refused(_)) => Self::Refused,
            Err(WriteError::Host(_)) => Self::HostError,
        }
    }
}

/// A node made at `level`, or Private when its creator picked none.
fn settle(born: &Born) -> Level {
    born.level.unwrap_or(Level::Private)
}

impl Writer {
    /// Runs Atta's generic checks on `act`, then has `host` apply it; a refused act reaches no
    /// host. Logs its outcome class, the act's word and its depth at debug.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn act<B: Send, P: Send>(
        &self,
        host: &mut dyn Host<Act = B, Applied = P>,
        act: Act<B>,
    ) -> Result<P, WriteError> {
        let word = act.name.clone();
        let depth = act.shape.depth();
        let written = self.act_unlogged(host, act).await;
        let outcome: &'static str = Outcome::from(&written).into();
        let refusal: Option<&'static str> = match &written {
            Err(WriteError::Refused(refusal)) => Some(refusal.into()),
            _ => None,
        };
        tracing::debug!(outcome, act = word.as_ref(), depth, refusal, "acted");
        written
    }

    /// [`Writer::act`]'s checks and apply, without its log line.
    async fn act_unlogged<B: Send, P: Send>(
        &self,
        host: &mut dyn Host<Act = B, Applied = P>,
        act: Act<B>,
    ) -> Result<P, WriteError> {
        let checked = self.check(&*host, act)?;
        let applied = host.apply(&self.viewer, checked).await?;
        Ok(applied)
    }

    /// Runs Atta's generic checks on `act` and nothing else: the refusal [`Writer::act`] would
    /// meet, or `()`. No host applies anything, and no [`Checked`] is made.
    pub fn dry_run<B: Send, P: Send>(
        &self,
        host: &dyn Host<Act = B, Applied = P>,
        act: &Act<B>,
    ) -> Result<(), Refusal> {
        self.settled(host, act).map(|_| ())
    }

    /// The five generic checks; the act, checked, or the first refusal.
    fn check<B: Send, P: Send>(
        &self,
        host: &dyn Host<Act = B, Applied = P>,
        act: Act<B>,
    ) -> Result<Checked<B>, Refusal> {
        let settled = self.settled(host, &act)?;
        let checked = Checked { act, settled };
        Ok(checked)
    }

    /// The five generic checks on `act`, by reference: each born node's settled level, or the
    /// first refusal.
    fn settled<B: Send, P: Send>(
        &self,
        host: &dyn Host<Act = B, Applied = P>,
        act: &Act<B>,
    ) -> Result<Vec<Level>, Refusal> {
        match &act.shape {
            Shape::Create {
                parent,
                node,
                entries,
            } => self.check_create(host, &act.typ, parent, node, entries),
            Shape::Rename { node, .. } | Shape::Change { node } => {
                if node.typ != act.typ {
                    return Err(Refusal::OtherType);
                }
                Ok(Vec::new())
            }
        }
    }

    /// The checks on a Create: its node is of the act's type; no node is made inside a file and
    /// a born file holds no entries; no born node lies deeper than the cap; each born node is
    /// settled (Private unless picked) and never wider than its parent; the entries' keys are
    /// distinct. The settled levels, the node's first.
    fn check_create<B: Send, P: Send>(
        &self,
        host: &dyn Host<Act = B, Applied = P>,
        act_type: &Type,
        parent: &Placed,
        node: &Born,
        entries: &[Born],
    ) -> Result<Vec<Level>, Refusal> {
        if &node.typ != act_type {
            return Err(Refusal::OtherType);
        }
        let node_is_file = host.kind(&node.typ) == Kind::File;
        if host.kind(&parent.typ) == Kind::File || (node_is_file && !entries.is_empty()) {
            return Err(Refusal::EntriesInFile);
        }
        let node_depth = parent.path.depth() + 1;
        let deepest = if entries.is_empty() {
            node_depth
        } else {
            node_depth + 1
        };
        if deepest > self.caps.max_real_depth {
            return Err(Refusal::TooDeep);
        }
        let node_level = settle(node);
        let entry_levels: Vec<Level> = entries.iter().map(settle).collect();
        let entry_wider = entry_levels.iter().any(|level| *level > node_level);
        if node_level > parent.level || entry_wider {
            return Err(Refusal::WiderThanParent);
        }
        let mut keys = HashSet::new();
        let distinct = entries.iter().all(|entry| keys.insert(&entry.key));
        if !distinct {
            return Err(Refusal::DuplicateKey);
        }
        let settled = std::iter::once(node_level).chain(entry_levels).collect();
        Ok(settled)
    }
}

impl fmt::Debug for Refusal {
    /// Its class.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class: &'static str = self.into();
        formatter.write_str(class)
    }
}

impl fmt::Debug for WriteError {
    /// Its refusal class, or `Host(..)`: a host's failure may carry a path or a name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => formatter.debug_tuple("Refused").field(refusal).finish(),
            Self::Host(_) => formatter.write_str("Host(..)"),
        }
    }
}

impl<B> fmt::Debug for Checked<B> {
    /// Its act's [`Debug`](fmt::Debug) and the settled levels.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Checked")
            .field("act", &self.act)
            .field("settled", &self.settled)
            .finish()
    }
}

impl fmt::Debug for Writer {
    /// Its caps; never its viewer.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Writer")
            .field("caps", &self.caps)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
