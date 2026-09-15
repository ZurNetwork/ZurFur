/// How much of what sits behind it a viewer class may see; borne by all three
/// composition terms (tab, surface, element).
///
/// Declaration order IS the openness ladder and the derived [`Ord`] is
/// load-bearing for [`effective_visibility`] — **do not reorder the variants**.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub enum VisibilityMode {
    /// Participants only; the default of every term.
    Total,
    /// Title + existence only — the status-only card tier.
    Presentation,
    /// Description-designated content — the widest tier.
    Description,
}

impl Default for VisibilityMode {
    /// [`Total`](Self::Total) — the closed door, matching every `mode` column's
    /// `DEFAULT 'total'` and the absent-surface-mode rule.
    fn default() -> Self {
        Self::Total
    }
}

impl VisibilityMode {
    ///
    /// The effective visibility of an element: `min(tab, surface, element)`.
    /// Over-claiming is inert, never a leak. Composes under the commission's own
    /// [`Visibility`](crate::elements::commission::Visibility), which the caller gates on first.
    ///
    /// ```
    /// use domain::elements::commission::VisibilityMode;
    ///
    /// // An element over-claiming under a closed surface stays closed.
    /// let effective = VisibilityMode::effective_visibility(
    ///     VisibilityMode::Description,
    ///     VisibilityMode::Total,
    ///     VisibilityMode::Description,
    /// );
    /// assert_eq!(effective, VisibilityMode::Total);
    /// ```
    pub fn effective_visibility(
        tab: VisibilityMode,
        surface: VisibilityMode,
        element: VisibilityMode,
    ) -> VisibilityMode {
        tab.min(surface).min(element)
    }
}

#[cfg(test)]
mod tests;
