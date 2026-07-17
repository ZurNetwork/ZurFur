//! The commission's content **tree** (ZMVP-71; Surfaces DD `28246028`, Tree
//! Storage DD `28409880`): nodes with a core-owned envelope and a type-owned
//! payload, stored as adjacency rows and loaded whole.
//!
//! Every node is `{id, type, envelope, payload, children}`. The envelope (id,
//! type, created_by, created_at, mode on surfaces) is what authorization and
//! audit read; the payload is opaque JSON the core never interprets (the type
//! catalog interprets it later). v1 grows the tree with **Surfaces** — interior,
//! visibility-bearing nodes — and **Components** (ZMVP-72) — leaves, always the
//! child of a surface, mode-less (they project with their parent), their
//! substance in the opaque payload.
//!
//! **The raw tree never serializes.** [`CommissionTree`]/[`CommissionNode`]
//! deliberately do **not** implement `serde::Serialize`, so "serialize the
//! loaded tree" is a compile error, not a review catch. The only way tree
//! content may leave the server is through the viewer-projected shape ZMVP-75
//! introduces (min-of-ancestors, computed server-side). Err closed, by
//! construction.

use std::collections::HashMap;
use std::ops::Deref;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::{Commission, CommissionId},
        user::UserId,
    },
};

/// The app-private, stable handle for one node in a commission's tree.
///
/// A UUID wrapped for type safety, mirroring [`CommissionId`]. Freshly grown
/// nodes mint UUIDv7 app-side; the roots backfilled by migration for
/// commissions that predate the tree carry `gen_random_uuid()` v4 keys (a
/// singleton root doesn't need time-sortability — `created_at` carries the real
/// time). `Deref` exposes the inner UUID for foreign keys and lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(uuid::Uuid);

impl NodeId {
    /// Wraps an already-minted UUID; the app mints the key, the domain only
    /// names it (mirroring [`CommissionId::new`]).
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh UUIDv7 node key (shared with the sibling shapes that grow
    /// the tree, e.g. [`NewSlot`](super::NewSlot)).
    pub(super) fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

impl Deref for NodeId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A Surface's visibility mode (Surfaces DD `28246028` Decision 2): how much of
/// the subtree under it a viewer class may see. Effective visibility is
/// **min-of-ancestors**, computed at read (ZMVP-75) — a child written wider than
/// its parent never *projects* wider.
///
/// The root surface's mode IS the commission-level visibility: the flat
/// `Private`/`Listed`/`Public` aliases map onto it (see
/// [`Visibility::as_root_mode`](super::Visibility::as_root_mode)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceMode {
    /// Title + existence only — the status-only card tier.
    Presentation,
    /// Description-designated content — what `Public` exposes.
    Description,
    /// Everything — participants-only. **The default of every new surface**
    /// (Surfaces DD Decision 4): widening is always explicit, per surface, so
    /// the platform errs closed.
    Total,
}

impl SurfaceMode {
    /// The stable, lowercase storage token — what the pg adapter writes to the
    /// `commission_node.mode` column. Persisted, so renaming a token is a
    /// migration, not a free edit.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Presentation => "presentation",
            Self::Description => "description",
            Self::Total => "total",
        }
    }

    /// Resolve a stored token back to its mode, or `None` for one outside the
    /// vocabulary — on a read path that means row tampering or a missed
    /// migration and surfaces as an error, never a silent default (the same
    /// contract as [`Visibility`](super::Visibility)'s `TryFrom<&str>`).
    pub fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "presentation" => Self::Presentation,
            "description" => Self::Description,
            "total" => Self::Total,
            _ => return None,
        })
    }
}

/// What a node *is* — the typed half of the envelope, carrying exactly the
/// mode-bearing rule of the Surfaces DD amendment by construction: a
/// **Surface always carries a mode** (it's inside the variant, not an `Option`
/// beside it), and a **Component carries none at all** (ZMVP-72) — its variant
/// simply has no mode field, so "a component inherits its parent's visibility"
/// is unrepresentable to violate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// An interior node: grouping/layout, visibility-bearing, may contain
    /// children (Surfaces and Components).
    Surface {
        /// The surface's own visibility mode; effective visibility is
        /// min-of-ancestors, computed at read (ZMVP-75).
        mode: SurfaceMode,
    },
    /// A leaf (ZMVP-72): always the child of a surface, never with children,
    /// projecting with its parent (no mode of its own). v1 is the generic,
    /// untyped contract — one tag for every component, the substance in the
    /// opaque payload; the type catalog (typed payload schemas, per-type
    /// behavior) is deliberately deferred per the Surfaces DD. Seats and Slots
    /// later couple onto this contract as (eventually typed) components.
    Component,
}

impl NodeKind {
    /// The stable `commission_node.type` storage tag for this kind.
    pub fn type_tag(&self) -> &'static str {
        match self {
            Self::Surface { .. } => "surface",
            Self::Component => "component",
        }
    }

    /// The surface mode this node carries, or `None` for kinds that inherit
    /// (Components). This is the value the nullable `mode` column stores.
    pub fn mode(&self) -> Option<SurfaceMode> {
        match self {
            Self::Surface { mode } => Some(*mode),
            Self::Component => None,
        }
    }

    /// Rebuild a kind from its stored `(type, mode)` column pair, or `None` for
    /// a pair outside the vocabulary — an unknown tag, a surface without a
    /// mode, a component *with* one, or a mode token nothing parses. On a read
    /// path any of those means row tampering or a missed migration and surfaces
    /// as an error, never a silent default.
    pub fn from_columns(type_tag: &str, mode: Option<&str>) -> Option<Self> {
        match (type_tag, mode) {
            ("surface", Some(token)) => Some(Self::Surface {
                mode: SurfaceMode::parse(token)?,
            }),
            ("component", None) => Some(Self::Component),
            _ => None,
        }
    }
}

/// A freshly grown surface, ready to persist under an existing parent
/// ([`CommissionWrites::add_surface`](crate::ports::CommissionWrites::add_surface)).
///
/// Built with [`NewSurface::under`], which is where AC3 lives: there is no mode
/// parameter — **every new surface is born [`SurfaceMode::Total`]** (Surfaces DD
/// Decision 4; widening is an explicit later act, ZMVP-74). Sibling `position`
/// is deliberately absent: the store assigns append order in-transaction.
#[derive(Debug)]
pub struct NewSurface {
    /// The freshly minted node key (UUIDv7).
    pub id: NodeId,
    /// The commission whose tree this grows. The store verifies `parent`
    /// belongs to this same commission.
    pub commission_id: CommissionId,
    /// The existing surface to grow under. Never `None` — the root surface is
    /// minted exactly once, with the commission itself (see [`RootSurface`]),
    /// so no caller can ever attempt a second root.
    pub parent: NodeId,
    /// The acting User (the owner; the route's authority gate settles that
    /// before this is built).
    pub created_by: UserId,
    /// When the surface was added.
    pub created_at: DateTimeUtc,
}

impl NewSurface {
    /// A new surface under `parent`. The birth mode is **inherited from the
    /// parent** at the store layer (Engineer ruling 2026-07-07, PR #103 —
    /// composing an open subtree must not require re-widening every new node;
    /// amends the Surfaces DD's born-`Total` default), so this carrier holds no
    /// mode at all: no caller can choose one. Inheritance never widens the
    /// tree — a child at its parent's mode exposes nothing the parent didn't
    /// already (and the min-of-ancestors cap holds regardless). The root is
    /// unaffected: it is minted `Total` with the commission ([`RootSurface`]),
    /// so a fresh commission still errs fully closed. Mints the node id;
    /// authority (owner-only in v1) is the caller's concern, settled before
    /// this is reached.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewSurface, NodeId},
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let parent = NodeId::new(uuid::Uuid::now_v7());
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let surface = NewSurface::under(commission, parent, owner, Utc::now());
    /// assert_eq!(surface.parent, parent); // mode: inherited at the store
    /// ```
    pub fn under(
        commission: CommissionId,
        parent: NodeId,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: NodeId::mint(),
            commission_id: commission,
            parent,
            created_by,
            created_at: now,
        }
    }
}

/// A freshly grown component — the tree's leaf — ready to persist under an
/// existing **surface**
/// ([`CommissionWrites::add_component`](crate::ports::CommissionWrites::add_component),
/// ZMVP-72).
///
/// Built with [`NewComponent::under`]. The envelope is core-owned (id, parent,
/// creator, instant); the `payload` is the type-owned half, carried **opaque**
/// — v1 is the generic, untyped contract, so the core neither validates nor
/// interprets it, and it round-trips unmodified (AC3). There is no mode field
/// anywhere: a component projects with its parent ([`NodeKind::Component`]).
/// Sibling `position` is deliberately absent: the store assigns append order
/// in-transaction, exactly as for [`NewSurface`].
#[derive(Debug)]
pub struct NewComponent {
    /// The freshly minted node key (UUIDv7).
    pub id: NodeId,
    /// The commission whose tree this grows. The store verifies `parent`
    /// belongs to this same commission.
    pub commission_id: CommissionId,
    /// The existing **surface** to grow under. Components are leaves: the store
    /// refuses a parent that is itself a component
    /// ([`ParentNotASurface`](crate::ports::ParentNotASurface)), so a component
    /// can never gain children (AC1/AC2).
    pub parent: NodeId,
    /// The type-owned payload, opaque to the core; stored and returned
    /// unmodified (AC3).
    pub payload: serde_json::Value,
    /// The acting User (the owner; the route's authority gate settles that
    /// before this is built).
    pub created_by: UserId,
    /// When the component was added.
    pub created_at: DateTimeUtc,
}

impl NewComponent {
    /// A new component under `parent`, carrying `payload` verbatim. Mints the
    /// node id; authority (owner-only in v1) and the parent-is-a-surface rule
    /// are the store's/route's concern, settled when this is persisted.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewComponent, NodeId},
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let parent = NodeId::new(uuid::Uuid::now_v7());
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let payload = serde_json::json!({ "kind": "text", "body": "hi" });
    /// let component = NewComponent::under(commission, parent, payload.clone(), owner, Utc::now());
    /// assert_eq!(component.payload, payload); // opaque, verbatim
    /// assert_eq!(component.parent, parent);
    /// ```
    pub fn under(
        commission: CommissionId,
        parent: NodeId,
        payload: serde_json::Value,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: NodeId::mint(),
            commission_id: commission,
            parent,
            payload,
            created_by,
            created_at: now,
        }
    }
}

/// The root surface a commission is **born with** (AC1: every commission has
/// one; Surfaces DD amendment 2). Derived from the commission itself and
/// persisted by [`CommissionWrites::create`](crate::ports::CommissionWrites::create)
/// in the same insert — a treeless commission is unrepresentable, and no second
/// code path exists that could forget it. It cannot be removed: pruning
/// (ZMVP-73) guards it —
/// [`CommissionWrites::remove_node`](crate::ports::CommissionWrites::remove_node)
/// refuses the root with
/// [`CannotRemoveRoot`](crate::ports::CannotRemoveRoot).
#[derive(Debug)]
pub struct RootSurface {
    /// The freshly minted node key (UUIDv7).
    pub id: NodeId,
    /// The root's mode — the commission-level visibility itself
    /// ([`Visibility::as_root_mode`](super::Visibility::as_root_mode)); a birth
    /// commission is `Private`, so its root is [`SurfaceMode::Total`].
    pub mode: SurfaceMode,
    /// The commission's owner.
    pub created_by: UserId,
    /// The commission's own creation instant — the root is born with it.
    pub created_at: DateTimeUtc,
}

impl RootSurface {
    /// The root surface for `commission`, mode mapped from its flat visibility
    /// (`Private`→`Total`, `Listed`→`Presentation`, `Public`→`Description`).
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{Commission, CommissionTitle, RootSurface, SurfaceMode},
    ///     user::UserId,
    /// };
    ///
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let title = "A ref sheet".parse::<CommissionTitle>().unwrap();
    /// let commission = Commission::create(title, owner, Utc::now(), None);
    /// let root = RootSurface::of(&commission);
    /// assert_eq!(root.mode, SurfaceMode::Total); // born Private = root Total
    /// assert_eq!(root.created_by, owner);
    /// assert_eq!(root.created_at, commission.created_at);
    /// ```
    pub fn of(commission: &Commission) -> Self {
        Self {
            id: NodeId::mint(),
            mode: commission.visibility.as_root_mode(),
            created_by: commission.owner_id,
            created_at: commission.created_at,
        }
    }
}

/// One stored node as a flat row — the adapter-neutral shape the whole-tree
/// load produces before [`CommissionTree::assemble`] turns rows into the tree
/// (Tree Storage DD Decision 4: one indexed query, assembly in Rust, not SQL).
#[derive(Debug)]
pub struct NodeRow {
    /// The node's key.
    pub id: NodeId,
    /// Its parent node, or `None` for the root (exactly one per commission).
    pub parent: Option<NodeId>,
    /// The typed envelope half: what the node is, and its mode if it bears one.
    pub kind: NodeKind,
    /// Sibling order within the parent (ascending; assembly sorts by it).
    pub position: i32,
    /// Who created the node.
    pub created_by: UserId,
    /// When the node was created.
    pub created_at: DateTimeUtc,
    /// The type-owned payload, opaque to the core.
    pub payload: serde_json::Value,
}

/// One assembled node of the loaded tree: envelope + payload + ordered
/// children. **Deliberately not `Serialize`** — see [`CommissionTree`].
#[derive(Debug)]
pub struct CommissionNode {
    /// The node's key.
    pub id: NodeId,
    /// The typed envelope half (kind + mode on surfaces).
    pub kind: NodeKind,
    /// Who created the node.
    pub created_by: UserId,
    /// When the node was created.
    pub created_at: DateTimeUtc,
    /// The type-owned payload, opaque to the core.
    pub payload: serde_json::Value,
    /// Child nodes in sibling order (their stored `position`, ascending). The
    /// order **is** this `Vec`'s order; positions don't survive assembly.
    pub children: Vec<CommissionNode>,
}

/// A commission's whole loaded content tree, rooted at its root surface —
/// the raw, unprojected read model
/// ([`CommissionStore::load_tree`](crate::ports::CommissionStore::load_tree)).
///
/// **This type must never serialize.** It holds everything — `Total`-tier
/// content included — so an impl of `serde::Serialize` here (or on
/// [`CommissionNode`]) would put "the whole tree leaves the server" one
/// `Json(tree)` away. Neither type implements it, so serializing the raw tree
/// is a **compile error** today; serialization exists only on the
/// viewer-projected shape ZMVP-75 introduces (min-of-ancestors, computed
/// server-side). Do not add a `Serialize` derive here — project first, always.
#[derive(Debug)]
pub struct CommissionTree {
    /// The root surface (parent `None`), children nested in sibling order.
    pub root: CommissionNode,
}

/// Why a set of stored rows failed to assemble into a tree. Any of these on a
/// read path means row tampering or a corrupted store — the constraints
/// (`one_root_per_commission`, the parent FK) make them unreachable through the
/// write ports — so they surface as errors, never a silent partial tree.
#[derive(Debug, PartialEq, Eq)]
pub enum TreeAssemblyError {
    /// No row with `parent = NULL`: the commission has rows but no root.
    NoRoot,
    /// More than one row with `parent = NULL`.
    MultipleRoots,
    /// Rows whose parent chain never reaches the root (an orphan or a cycle).
    Detached,
}

impl std::fmt::Display for TreeAssemblyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoot => write!(f, "commission tree has no root node"),
            Self::MultipleRoots => write!(f, "commission tree has more than one root node"),
            Self::Detached => write!(
                f,
                "commission tree has nodes detached from the root (orphan or cycle)"
            ),
        }
    }
}

impl std::error::Error for TreeAssemblyError {}

impl CommissionTree {
    /// Assemble flat stored rows into the nested tree: find the single root,
    /// attach every row under its parent, and order each sibling group by
    /// `position` ascending (Tree Storage DD Decision 4 — assembly in Rust).
    /// Rows that never attach — orphans or cycles — are
    /// [`Detached`](TreeAssemblyError::Detached), never silently dropped.
    pub fn assemble(rows: Vec<NodeRow>) -> Result<Self, TreeAssemblyError> {
        let mut roots = Vec::new();
        let mut children_of: HashMap<NodeId, Vec<NodeRow>> = HashMap::new();
        for row in rows {
            match row.parent {
                None => roots.push(row),
                Some(parent) => children_of.entry(parent).or_default().push(row),
            }
        }
        let root = match (roots.pop(), roots.is_empty()) {
            (Some(root), true) => root,
            (Some(_), false) => return Err(TreeAssemblyError::MultipleRoots),
            (None, _) => return Err(TreeAssemblyError::NoRoot),
        };

        let root = Self::attach(root, &mut children_of);
        if !children_of.is_empty() {
            return Err(TreeAssemblyError::Detached);
        }
        Ok(Self { root })
    }

    /// Turn `row` into a node, recursively claiming its children from the
    /// parent index (removal is what lets [`assemble`](Self::assemble) detect
    /// leftovers as detached). Depth is bounded by the tree the owner built —
    /// dozens of nodes by design (Tree Storage DD).
    fn attach(row: NodeRow, children_of: &mut HashMap<NodeId, Vec<NodeRow>>) -> CommissionNode {
        let mut rows = children_of.remove(&row.id).unwrap_or_default();
        rows.sort_by_key(|child| child.position);
        CommissionNode {
            id: row.id,
            kind: row.kind,
            created_by: row.created_by,
            created_at: row.created_at,
            payload: row.payload,
            children: rows
                .into_iter()
                .map(|child| Self::attach(child, children_of))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::elements::commission::{CommissionTitle, Visibility};

    fn row(
        id: NodeId,
        parent: Option<NodeId>,
        mode: SurfaceMode,
        position: i32,
        by: UserId,
    ) -> NodeRow {
        NodeRow {
            id,
            parent,
            kind: NodeKind::Surface { mode },
            position,
            created_by: by,
            created_at: Utc::now(),
            payload: json!({}),
        }
    }

    // AC3/AC4 as amended (Engineer ruling 2026-07-07, PR #103) — a new
    // surface's envelope: fresh id, the parent it grows under, the acting
    // user, and NO mode field at all: the mode is inherited from the parent at
    // the store layer, so no caller can choose one (the adapters' tests pin
    // the inheritance itself).
    #[test]
    fn a_new_surface_carries_no_mode_of_its_own() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let parent = NodeId::new(uuid::Uuid::now_v7());
        let owner = UserId::new(uuid::Uuid::now_v7());

        let surface = NewSurface::under(commission, parent, owner, Utc::now());

        assert_eq!(surface.parent, parent);
        assert_eq!(surface.commission_id, commission);
        assert_eq!(surface.created_by, owner);
    }

    // AC1 — the root surface derives from the commission: mode maps from the
    // flat visibility exactly as the Surfaces DD amendment aliases it.
    #[test]
    fn the_root_mode_is_the_commission_visibility() {
        let owner = UserId::new(uuid::Uuid::now_v7());
        let title = "Aliases".parse::<CommissionTitle>().unwrap();
        let mut commission = Commission::create(title, owner, Utc::now(), None);

        // Birth: Private = root Total.
        assert_eq!(RootSurface::of(&commission).mode, SurfaceMode::Total);
        commission.visibility = Visibility::Listed;
        assert_eq!(RootSurface::of(&commission).mode, SurfaceMode::Presentation);
        commission.visibility = Visibility::Public;
        assert_eq!(RootSurface::of(&commission).mode, SurfaceMode::Description);

        let root = RootSurface::of(&commission);
        assert_eq!(root.created_by, owner, "the owner is the root's creator");
        assert_eq!(
            root.created_at, commission.created_at,
            "the root is born with the commission"
        );
    }

    // ZMVP-72 AC2/AC3 — a new component's envelope: fresh id, the surface it
    // grows under, the acting user, its opaque payload carried verbatim — and
    // NO mode anywhere (the Component variant has none to set; it projects
    // with its parent).
    #[test]
    fn a_new_component_carries_its_payload_and_no_mode() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let parent = NodeId::new(uuid::Uuid::now_v7());
        let owner = UserId::new(uuid::Uuid::now_v7());
        let payload = json!({ "kind": "text", "body": "Reference: 三毛猫 🐾", "revision": 3 });

        let component = NewComponent::under(commission, parent, payload.clone(), owner, Utc::now());

        assert_eq!(component.payload, payload, "the payload is carried opaque");
        assert_eq!(component.parent, parent);
        assert_eq!(component.commission_id, commission);
        assert_eq!(component.created_by, owner);
    }

    // ZMVP-72 AC2 — the component kind bears no mode by construction and maps
    // to the 'component' type tag with a NULL mode column.
    #[test]
    fn a_component_kind_has_no_mode() {
        let kind = NodeKind::Component;
        assert_eq!(kind.type_tag(), "component");
        assert_eq!(kind.mode(), None, "a component carries no mode of its own");
        assert_eq!(NodeKind::from_columns("component", None), Some(kind));
        assert_eq!(
            NodeKind::from_columns("component", Some("total")),
            None,
            "a component WITH a mode is tampering"
        );
    }

    // Envelope round-trip: the stored (type, mode) column pair rebuilds the
    // kind; anything outside the vocabulary is refused (tamper surfacing).
    #[test]
    fn node_kind_round_trips_and_refuses_tampering() {
        let kind = NodeKind::Surface {
            mode: SurfaceMode::Description,
        };
        assert_eq!(kind.type_tag(), "surface");
        assert_eq!(kind.mode(), Some(SurfaceMode::Description));
        assert_eq!(
            NodeKind::from_columns("surface", Some("description")),
            Some(kind)
        );

        assert_eq!(
            NodeKind::from_columns("surface", None),
            None,
            "a surface without a mode is tampering"
        );
        assert_eq!(
            NodeKind::from_columns("surface", Some("wide-open")),
            None,
            "an unknown mode token is tampering"
        );
        assert_eq!(
            NodeKind::from_columns("gizmo", Some("total")),
            None,
            "an unknown type tag is tampering"
        );
    }

    // Assembly: rows nest under their parents with each sibling group ordered
    // by position ascending, regardless of row arrival order.
    #[test]
    fn assemble_nests_and_orders_siblings_by_position() {
        let by = UserId::new(uuid::Uuid::now_v7());
        let root = NodeId::new(uuid::Uuid::now_v7());
        let first = NodeId::new(uuid::Uuid::now_v7());
        let second = NodeId::new(uuid::Uuid::now_v7());
        let nested = NodeId::new(uuid::Uuid::now_v7());

        // Deliberately shuffled: children before the root, positions reversed.
        let tree = CommissionTree::assemble(vec![
            row(second, Some(root), SurfaceMode::Total, 1, by),
            row(nested, Some(first), SurfaceMode::Total, 0, by),
            row(root, None, SurfaceMode::Total, 0, by),
            row(first, Some(root), SurfaceMode::Total, 0, by),
        ])
        .expect("a well-formed tree assembles");

        assert_eq!(tree.root.id, root);
        assert_eq!(tree.root.children.len(), 2);
        assert_eq!(
            tree.root.children[0].id, first,
            "siblings order by position"
        );
        assert_eq!(tree.root.children[1].id, second);
        assert_eq!(tree.root.children[0].children[0].id, nested, "nesting");
        assert!(tree.root.children[1].children.is_empty());
    }

    // Corruption surfaces as an error, never a silent partial tree.
    #[test]
    fn assemble_refuses_corrupt_row_sets() {
        let by = UserId::new(uuid::Uuid::now_v7());
        let a = NodeId::new(uuid::Uuid::now_v7());
        let b = NodeId::new(uuid::Uuid::now_v7());

        assert_eq!(
            CommissionTree::assemble(vec![]).unwrap_err(),
            TreeAssemblyError::NoRoot
        );
        assert_eq!(
            CommissionTree::assemble(vec![
                row(a, None, SurfaceMode::Total, 0, by),
                row(b, None, SurfaceMode::Total, 0, by),
            ])
            .unwrap_err(),
            TreeAssemblyError::MultipleRoots
        );
        // An orphan: b's parent is a node that isn't in the set.
        let stray = NodeId::new(uuid::Uuid::now_v7());
        assert_eq!(
            CommissionTree::assemble(vec![
                row(a, None, SurfaceMode::Total, 0, by),
                row(b, Some(stray), SurfaceMode::Total, 0, by),
            ])
            .unwrap_err(),
            TreeAssemblyError::Detached
        );
    }
}
