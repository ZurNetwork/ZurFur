-- The deadline-axis Status: how the work
-- stands against its deadline. ONE nullable column holding the MANUAL Participant
-- flag only — `delayed` or NULL. **`Late` is never persisted**: a commission whose
-- deadline has passed *is* Late, derived fresh on
-- every lookup from `deadline < now` (and its lifecycle not terminal) and, at
-- most, logged once to the changelog by the deadline sweep. The CHECK below makes
-- a persisted `late` unrepresentable — the axis column is the flag, nothing else.
-- A commission with no deadline never carries a value here. The direction
-- axis is the sibling column; the two compose freely.
ALTER TABLE commission
    ADD COLUMN deadline_status text
        CONSTRAINT commission_deadline_status_delayed_only
        CHECK (deadline_status IS NULL OR deadline_status = 'delayed');

-- The deadline sweep scans `deadline < now` to LOG the derived Late transition
-- (it never persists Late). A partial btree index over the only rows that can be
-- late keeps that periodic scan off a full-table scan as the table grows. (This
-- is the stopgap; scaling the sweep further is deferred to a later change.)
CREATE INDEX commission_deadline_pending_idx
    ON commission (deadline)
    WHERE deadline IS NOT NULL;
