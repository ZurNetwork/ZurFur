-- A Disputed commission keeps the step it left and when the dispute began, so a
-- clear returns it to exactly that step (Draft included) and its deadline's
-- clock stands still from the dispute's start. Both columns are set together
-- while it is Disputed and empty on every other step. No row is written. Why
-- the file is shaped this way: this crate's NODE.inf.json notes, under this
-- file.

-- No commission is written while the check and the new columns land.
LOCK TABLE commission IN SHARE ROW EXCLUSIVE MODE;

-- A commission already Disputed has no step to clear back to: the migration
-- stops, naming its id alone. Nothing is repaired.
DO $$
DECLARE
    offender uuid;
BEGIN
    SELECT c.id INTO offender FROM commission c
    WHERE c.lifecycle = 'disputed'
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission dispute columns stop: commission % is disputed with no step to return to', offender;
    END IF;
END
$$;

ALTER TABLE commission ADD COLUMN disputed_from text;
ALTER TABLE commission ADD COLUMN disputed_at timestamptz;

-- The dispute stands exactly while the commission is Disputed, its time is set
-- with its step, and the step it left is one of the other five.
ALTER TABLE commission ADD CONSTRAINT commission_dispute_matches_its_step CHECK (
    (lifecycle = 'disputed') = (disputed_from IS NOT NULL)
    AND (disputed_from IS NULL) = (disputed_at IS NULL)
    AND (disputed_from IS NULL OR disputed_from IN ('draft', 'batched', 'active', 'completed', 'cancelled'))
);
