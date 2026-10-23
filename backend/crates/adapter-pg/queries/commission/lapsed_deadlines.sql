-- The deadline sweep's scan on the pool: the deadline param receives the sweep
-- instant ("now"); lifecycle takes the steps a missed deadline doesn't lapse on
-- (the terminal ones and Disputed). A commission with a `late` entry recorded
-- since its deadline last changed is left out. Only a scan: the sweep re-reads
-- each one under its own lock before it writes.
SELECT c.id, c.deadline, c.deadline_status
FROM commission c
WHERE c.deadline IS NOT NULL
  AND c.deadline < $1
  AND NOT (c.lifecycle = ANY($2))
  AND NOT EXISTS (
      SELECT 1 FROM commission_changelog e
      WHERE e.commission_id = c.id
        AND e.kind = 'late'
        AND e.seq > COALESCE((
            SELECT MAX(d.seq) FROM commission_changelog d
            WHERE d.commission_id = c.id
              AND d.kind IN ('deadline_set', 'deadline_extended')
        ), 0)
  )
ORDER BY c.deadline, c.id
