-- Contribute one element into a declared surface (ZMVP-166). The single insert
-- path: the generic element add, a Slot's carrying element, and a Seat's all
-- come through here, differing only in the `type` tag and payload they bind, so
-- there is no second place an element can be born.
--
-- `position` is assigned in the same statement as max + 1 WITHIN THE BAND —
-- (commission, tab, surface, band) — so append order cannot race; the caller
-- holds the tab's row lock (require_tab.sql) for the duration.
INSERT INTO commission_element
    (id, commission_id, tab_id, surface, type, mode, band, position, created_by, created_at, payload)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    (SELECT COALESCE(MAX(position) + 1, 0) FROM commission_element
     WHERE commission_id = $2 AND tab_id = $3 AND surface = $4 AND band = $7),
    $8, $9, $10
)
