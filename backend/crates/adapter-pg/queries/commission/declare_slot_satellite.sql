-- The declared Slot's interpreted half, keyed by the carrying
-- element's id — one identity, two rows. Deliberately no
-- occupant column of any kind: fill is the Character epic's.
INSERT INTO commission_slot (element_id, commission_id, title, notes)
VALUES ($1, $2, $3, $4)
