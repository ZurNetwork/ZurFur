-- Drop the superseded account-level placement rails (Engineer ruling
-- 2026-09-10: "Placement gets deleted").
--
-- `commission_placement` and its denormalized `commission_current_placement`
-- pointer were an append-only log with exactly one CURRENT account per
-- commission. That is the shape of the **managing-account assignment log** the
-- Ownership Separation DD (DESIGN/29130754) superseded — its own Context section
-- names it: "the recorded model was 1:1-current (append-only log, 'current
-- managing account = latest row,' transfer, reach-follows-manager)". The DD
-- replaced it with the two rails, and put placement itself on the board:
-- Decision 6, "**Placement = workflow membership rows, account-side**".
--
-- So placement was never a second thing. It is a card in a column
-- (`workflow_column_commission`, created in `20260910185856`), which is also why
-- one commission sits on N accounts' boards with no conflict — the NxM intent
-- the DD calls native, and which a one-row-per-commission pointer could not
-- express.
--
-- Nothing reads or writes these tables any more: `CommissionWrites::place` and
-- the `current_placement`/`placement_log` reads are gone from the port, both
-- adapters, and the query set. This drop is the storage half of that removal.
--
-- **Drop, not migrate.** Pre-alpha, and there is nothing to carry across: a row
-- here says "account A is the current placement of commission C", which has no
-- referent in the new model — it names no board and no column, and inventing one
-- would be fabricating positioning the artist never arranged.

DROP TABLE commission_current_placement;
DROP TABLE commission_placement;
