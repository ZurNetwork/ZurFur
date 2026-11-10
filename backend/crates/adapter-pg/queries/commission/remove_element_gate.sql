-- The removal gate: the target must exist in THIS commission — an
-- absent element id and one belonging to another commission both match nothing,
-- indistinguishably (ElementNotFound), so removal probes reveal nothing about
-- other commissions. An element that carries a Slot matches nothing too, so this
-- removal never takes a Slot, or its node, with it. Returns the ordering group the
-- removal will have to renumber — and, in `tab_id`, the row the caller must LOCK
-- (require_tab.sql) before it deletes anything.
--
-- Deliberately NOT `FOR UPDATE`: this reads an element to learn which TAB to
-- lock, and the three columns it reads (tab_id, surface, band) are immutable for
-- the life of a row — no statement anywhere updates them, only `position`
-- moves. So a lock-free read here cannot go stale in a way that matters, and
-- taking the tab lock afterwards still serializes the delete + renumber against
-- every concurrent append. If the element itself disappears in that window, the
-- DELETE affects zero rows and the removal re-refuses as ElementNotFound. A Slot is
-- only ever declared with a new element, so an element this gate passes never
-- gains one before its DELETE.
SELECT element.tab_id, element.surface, element.band FROM commission_element AS element
WHERE element.id = $1 AND element.commission_id = $2
  AND NOT EXISTS (SELECT 1 FROM commission_slot AS slot WHERE slot.element_id = element.id)
