-- Mint one of the commission's skeleton tabs, in the same unit of work as the
-- commission row itself (ZMVP-166; Flat Composition DD 45514754). Tab state
-- exists explicitly from birth — the withheld-at-birth discipline — so absence
-- of a tab row never has to mean anything. `mode` is bound rather than left to
-- the column DEFAULT so the closed door is stated by the code that mints it,
-- not inferred from schema.
INSERT INTO commission_tab (id, commission_id, tab, mode)
VALUES ($1, $2, $3, $4)
