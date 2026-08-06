-- The removal gate (ZMVP-166): the target must exist in THIS commission — an
-- absent element id and one belonging to another commission both match nothing,
-- indistinguishably (ElementNotFound), so removal probes reveal nothing about
-- other commissions. Returns the ordering group the removal will have to
-- renumber — and, in `tab_id`, the row the caller must LOCK (require_tab.sql)
-- before it deletes anything.
--
-- Deliberately NOT `FOR UPDATE`: this reads an element to learn which TAB to
-- lock, and the three columns it reads (tab_id, surface, band) are immutable for
-- the life of a row — no statement anywhere updates them, only `position`
-- moves. So a lock-free read here cannot go stale in a way that matters, and
-- taking the tab lock afterwards still serializes the delete + renumber against
-- every concurrent append. If the element itself disappears in that window, the
-- DELETE affects zero rows and the removal re-refuses as ElementNotFound.
SELECT tab_id, surface, band FROM commission_element
WHERE id = $1 AND commission_id = $2
