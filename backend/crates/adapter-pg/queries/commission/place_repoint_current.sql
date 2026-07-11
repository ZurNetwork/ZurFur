INSERT INTO commission_current_placement (commission_id, account_id, seq, placed_by, placed_at)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (commission_id)
DO UPDATE SET account_id = EXCLUDED.account_id,
              seq = EXCLUDED.seq,
              placed_by = EXCLUDED.placed_by,
              placed_at = EXCLUDED.placed_at
