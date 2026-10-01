use super::{SurfaceName, TabName};

/// One tab declared by the [`SKELETON`]: its stable name and the surfaces that
/// live in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredTab {
    /// The tab's stable id — the token minted into `commission_tab.tab`.
    pub tab: &'static str,
    /// The surfaces declared inside this tab, in render order.
    pub surfaces: &'static [&'static str],
}

/// The code-declared composition skeleton: which tabs exist, and which surfaces
/// live in each. Global and invariant — no write creates, renames, or removes a
/// surface.
///
/// ⚠️ Placeholder scaffolding, not a decision: the real skeleton is the type
/// catalog's. The names below carry no meaning worth preserving.
pub const SKELETON: &[DeclaredTab] = &[DeclaredTab {
    tab: "main",
    surfaces: &["content"],
}];

/// Whether the skeleton declares `surface` inside `tab` — the fail-closed
/// vocabulary check both adapters run before writing an element
/// ([`UnknownSurface`](crate::ports::UnknownSurface)).
///
/// The (tab, surface) pair is the unit: a surface declared under another tab is
/// refused exactly like an invented name.
pub fn declares_surface(tab: &TabName, surface: &SurfaceName) -> bool {
    SKELETON
        .iter()
        .find(|declared| declared.tab == tab.as_ref())
        .is_some_and(|declared| declared.surfaces.contains(&surface.as_ref()))
}

/// Every tab the skeleton declares, as validated [`TabName`]s — what
/// [`CommissionWrites::create`](crate::ports::CommissionWrites::create) mints a
/// row for, so a commission's tab state exists explicitly from birth.
///
/// Panics if the skeleton holds a malformed label (a programming error in the
/// const above).
pub fn declared_tabs() -> Vec<TabName> {
    SKELETON
        .iter()
        .map(|declared| {
            declared
                .tab
                .parse::<TabName>()
                .expect("SKELETON declares a malformed tab name")
        })
        .collect()
}

#[cfg(test)]
mod tests;
