use super::MaturityRating;

/// A work's complete maturity posture: the [`MaturityRating`] plus the
/// orthogonal Graphic flag, which rides alongside any rating rather than being a
/// fifth tier. One value by design — a work is either unrated or carries both,
/// so a graphic flag without a rating is unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Maturity {
    /// The four-tier axis value.
    pub rating: MaturityRating,
    /// Whether the work is graphic (gore/violence) — orthogonal to the axis.
    pub graphic: bool,
}

impl Maturity {
    /// The self-label the Graphic flag emits.
    pub const GRAPHIC_LABEL: &'static str = "graphic-media";

    /// Every atproto self-label this posture emits at publish time. Empty
    /// means Safe and not graphic. Publish paths wrap these into
    /// [`SelfLabels`](crate::elements::public_record::SelfLabels); commissions
    /// never do.
    pub fn self_labels(&self) -> Vec<&'static str> {
        let mut labels: Vec<&'static str> = self.rating.self_label().into_iter().collect();
        if self.graphic {
            labels.push(Self::GRAPHIC_LABEL);
        }
        labels
    }
}

#[cfg(test)]
mod tests;
