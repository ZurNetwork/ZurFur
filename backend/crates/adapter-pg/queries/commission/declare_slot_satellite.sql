-- The declared Slot's interpreted half (ZMVP-77), keyed by the carrying
-- element's id — one identity, two rows (Gate A ruling E20). Deliberately no
-- occupant column of any kind: fill is the Character epic's.
INSERT INTO commission_slot (element_id, commission_id, title, notes)
VALUES ($1, $2, $3, $4)
