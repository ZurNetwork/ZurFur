-- The shared TAB GATE — and the one SERIALIZATION POINT — of every composition
-- write (ZMVP-166): the named tab must exist in this commission, and its row is
-- locked for the rest of the transaction.
--
-- Existence: an absent id and a tab belonging to another commission both match
-- nothing, indistinguishably, so a probe reveals nothing about other
-- commissions (UnknownTab). This is the friendly half of a rule the schema
-- already enforces: the composite foreign key (tab_id, commission_id) ->
-- commission_tab (id, commission_id) makes a cross-commission element unwritable
-- regardless. The gate exists so the route can answer 404 instead of surfacing a
-- constraint violation.
--
-- Vocabulary: `tab` comes back because the surface check is on the PAIR — the
-- skeleton declares which surfaces live in which tab, and only the tab's
-- DECLARED NAME (never its per-commission id) can answer that. The adapter
-- re-validates the stored token into a TabName before consulting the skeleton.
--
-- FOR UPDATE locks the tab row, so every write that touches this tab's ordering
-- groups SERIALIZES: concurrent appends cannot race to one (surface, band,
-- position) slot and abort on the deferred UNIQUE at commit (the PR #103
-- hardening, carried over from the retired parent gate), and a concurrent
-- removal's renumbering cannot interleave with an append into the same group.
-- BOTH write paths take this lock, and take it before touching any element row.
SELECT id, tab FROM commission_tab
WHERE id = $1 AND commission_id = $2
FOR UPDATE
