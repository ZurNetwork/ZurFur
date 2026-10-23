-- Whether one commission is still owed a `late` entry: none recorded since its
-- deadline last changed. The sweep scan's anti-join, for one commission, run on
-- the unit after its lock so it sees what committed while the lock was awaited.
SELECT NOT EXISTS (
    SELECT 1 FROM commission_changelog e
    WHERE e.commission_id = $1
      AND e.kind = 'late'
      AND e.seq > COALESCE((
          SELECT MAX(d.seq) FROM commission_changelog d
          WHERE d.commission_id = $1
            AND d.kind IN ('deadline_set', 'deadline_extended')
      ), 0)
) AS late_owed
