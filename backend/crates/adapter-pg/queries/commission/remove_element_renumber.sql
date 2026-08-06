-- Renumber a vacated ordering group to contiguous positions from 0, preserving
-- order (ZMVP-166). The group is (commission, tab, surface, band) — the same
-- tuple the append counts within and the deferred UNIQUE covers. Scoped by
-- commission_id so the statement never leans on tab_id uniqueness alone as its
-- scoping mechanism (PR #109 review). The UNIQUE is DEFERRABLE, so the
-- intermediate collisions this UPDATE passes through cannot trip it inside the
-- transaction.
UPDATE commission_element AS element
SET position = renumbered.position
FROM (
    SELECT id, (ROW_NUMBER() OVER (ORDER BY position))::int - 1 AS position
    FROM commission_element
    WHERE commission_id = $1 AND tab_id = $2 AND surface = $3 AND band = $4
) AS renumbered
WHERE element.id = renumbered.id AND element.position <> renumbered.position
