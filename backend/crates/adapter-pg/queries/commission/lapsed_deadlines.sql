-- params: now, terminal_lifecycles
-- fetch: many
-- row: LapsedRow
-- not_null: deadline
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
