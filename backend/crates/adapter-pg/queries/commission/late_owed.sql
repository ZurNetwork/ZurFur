-- Whether one commission is still owed a `late` entry: no entry of mark_kind
-- (the kind a Late mark records) since its last entry of rearm_kind (the kinds
-- that re-arm it). The sweep scan's anti-join, for one commission, run on the
-- unit after its lock so it sees what committed while the lock was awaited.
SELECT NOT EXISTS (
    SELECT 1 FROM commission_changelog mark
    WHERE mark.commission_id = $1
      AND mark.kind = $2
      AND mark.seq > COALESCE((
          SELECT MAX(rearm.seq) FROM commission_changelog rearm
          WHERE rearm.commission_id = $1
            AND rearm.kind = ANY($3)
      ), 0)
) AS late_owed
