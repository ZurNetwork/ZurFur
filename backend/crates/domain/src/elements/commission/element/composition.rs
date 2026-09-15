use std::collections::HashMap;

use super::{ElementRow, SurfaceName, TabId, TabRow, VisibilityMode};

/// A commission's whole loaded composition — every tab, every widened surface
/// mode, and every element
/// ([`CommissionStore::load_composition`](crate::ports::CommissionStore::load_composition)).
/// All three travel together because [`effective_visibility`] needs all three
/// terms.
///
/// **Never add a `Serialize` derive here.** It holds `Total`-tier content;
/// serialization exists only on a viewer projection that has already applied
/// [`effective_visibility_of`](Self::effective_visibility_of).
#[derive(Debug)]
pub struct CommissionComposition {
    /// The commission's tabs (its skeleton rows), ordered by declared name.
    pub tabs: Vec<TabRow>,
    /// The per-commission surface-mode overrides; sparse — an absent entry
    /// means [`VisibilityMode::Total`].
    pub surface_modes: HashMap<SurfaceName, VisibilityMode>,
    /// Every element, ordered by `(tab, surface, band, position)`.
    pub elements: Vec<ElementRow>,
}

impl CommissionComposition {
    /// The mode of the tab `id` names, or `None` if this composition holds no
    /// such tab (corruption, not a supported case).
    pub fn tab_mode(&self, id: TabId) -> Option<VisibilityMode> {
        self.tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.mode)
    }

    /// The mode of `surface` for this commission — the override if one was
    /// written, else [`VisibilityMode::Total`].
    pub fn surface_mode(&self, surface: &SurfaceName) -> VisibilityMode {
        self.surface_modes
            .get(surface)
            .copied()
            .unwrap_or(VisibilityMode::Total)
    }

    /// The effective visibility of one of this composition's elements —
    /// [`effective_visibility`] with the three terms resolved from here. An
    /// element whose tab is absent projects [`VisibilityMode::Total`].
    pub fn effective_visibility_of(&self, element: &ElementRow) -> VisibilityMode {
        let tab = self
            .tab_mode(element.address.tab)
            .unwrap_or(VisibilityMode::Total);
        let surface = self.surface_mode(&element.address.surface);
        VisibilityMode::effective_visibility(tab, surface, element.mode)
    }
}

#[cfg(test)]
mod tests;
