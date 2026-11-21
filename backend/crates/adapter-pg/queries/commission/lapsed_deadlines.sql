-- The deadline sweep's scan on the pool: the deadline param receives the sweep
-- instant ("now"); lifecycle takes the steps a missed deadline doesn't lapse on
-- (the terminal ones and Disputed); mark_kind takes the kind a Late mark records,
-- and rearm_kind the kinds that re-arm it. A commission with a mark recorded
-- since its last re-arming entry is left out. Answers ids only, ordered by
-- deadline, then id. Only a scan: the sweep re-reads each one under its own lock
-- before it writes.
SELECT c.id
FROM commission c
WHERE c.deadline IS NOT NULL
  AND c.deadline < $1
  AND NOT (c.lifecycle = ANY($2))
  AND NOT EXISTS (
      SELECT 1 FROM commission_changelog mark
      WHERE mark.commission_id = c.id
        AND mark.kind = $3
        AND mark.seq > COALESCE((
            SELECT MAX(rearm.seq) FROM commission_changelog rearm
            WHERE rearm.commission_id = c.id
              AND rearm.kind = ANY($4)
        ), 0)
  )
ORDER BY c.deadline, c.id
