use std::fmt;

use crate::Segment;

/// Where a node really lives: its segments from the global root `/`, which is the empty path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, derive_more::From)]
pub struct RealPath(Vec<Segment>);

impl RealPath {
    /// The path one segment below this one.
    pub fn join(&self, segment: Segment) -> Self {
        let mut segments = self.0.clone();
        segments.push(segment);
        Self(segments)
    }

    /// Its segments, the top-level directory first.
    pub fn segments(&self) -> &[Segment] {
        &self.0
    }

    /// How many segments below `/` it sits; a top-level directory is at depth 1.
    pub fn depth(&self) -> usize {
        self.0.len()
    }
}

impl FromIterator<Segment> for RealPath {
    fn from_iter<I: IntoIterator<Item = Segment>>(segments: I) -> Self {
        Self(segments.into_iter().collect())
    }
}

impl fmt::Display for RealPath {
    /// `/` for the root, else each segment after a `/`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return formatter.write_str("/");
        }
        for segment in &self.0 {
            write!(formatter, "/{segment}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
