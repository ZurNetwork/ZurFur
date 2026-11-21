-- A commission holds only the words its readers know and only the instants the
-- wire can carry: its lifecycle, direction status and rating are each one of
-- their words, and its four times lie in the years 1 to 9999 UTC, which also
-- refuses an infinite time. A row already breaking a rule stops the migration,
-- naming its id alone; no row is written or repaired and the table is not
-- rewritten. Why the file is shaped this way: this crate's NODE.inf.json notes,
-- under this file.

-- Nothing reads or writes a commission while the checks and the constraints
-- land, so no row slips in between them and no lock is upgraded.
LOCK TABLE commission IN ACCESS EXCLUSIVE MODE;

-- Each rule refuses the first row, by id, its constraint would refuse: the
-- predicate is the constraint's own expression, negated. Each refusal names an
-- id alone, never the stored word or time. Nothing is repaired.
DO $$
DECLARE
    offender uuid;
BEGIN
    SELECT c.id INTO offender FROM commission c
    WHERE NOT (c.lifecycle IN ('draft', 'batched', 'active', 'completed', 'cancelled', 'disputed'))
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission checks stop: commission % holds a lifecycle no reader knows', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE NOT (c.direction_status IS NULL
               OR c.direction_status IN ('waiting_for_input', 'waiting_for_approval', 'changes_requested'))
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission checks stop: commission % holds a direction status no reader knows', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE NOT (c.maturity IS NULL OR c.maturity IN ('safe', 'suggestive', 'nudity', 'adult'))
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission checks stop: commission % holds a rating no reader knows', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE NOT (c.created_at >= '0001-01-01 00:00:00+00'
               AND c.created_at <= '9999-12-31 23:59:59.999999+00')
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission checks stop: commission % has a creation time outside years 1 to 9999', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE NOT (c.deadline IS NULL
               OR (c.deadline >= '0001-01-01 00:00:00+00'
                   AND c.deadline <= '9999-12-31 23:59:59.999999+00'))
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission checks stop: commission % has a deadline outside years 1 to 9999', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE NOT (c.archived_at IS NULL
               OR (c.archived_at >= '0001-01-01 00:00:00+00'
                   AND c.archived_at <= '9999-12-31 23:59:59.999999+00'))
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission checks stop: commission % has an archive time outside years 1 to 9999', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE NOT (c.disputed_at IS NULL
               OR (c.disputed_at >= '0001-01-01 00:00:00+00'
                   AND c.disputed_at <= '9999-12-31 23:59:59.999999+00'))
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission checks stop: commission % has a dispute time outside years 1 to 9999', offender;
    END IF;
END
$$;

-- Every constraint in one statement, so the table is scanned once to validate
-- them. Each bound carries its zone, so the session's time zone never moves it.
ALTER TABLE commission
    ADD CONSTRAINT commission_lifecycle_known CHECK (
        lifecycle IN ('draft', 'batched', 'active', 'completed', 'cancelled', 'disputed')
    ),
    ADD CONSTRAINT commission_direction_status_known CHECK (
        direction_status IS NULL
        OR direction_status IN ('waiting_for_input', 'waiting_for_approval', 'changes_requested')
    ),
    ADD CONSTRAINT commission_maturity_known CHECK (
        maturity IS NULL OR maturity IN ('safe', 'suggestive', 'nudity', 'adult')
    ),
    ADD CONSTRAINT commission_created_at_in_range CHECK (
        created_at >= '0001-01-01 00:00:00+00'
        AND created_at <= '9999-12-31 23:59:59.999999+00'
    ),
    ADD CONSTRAINT commission_deadline_in_range CHECK (
        deadline IS NULL
        OR (deadline >= '0001-01-01 00:00:00+00' AND deadline <= '9999-12-31 23:59:59.999999+00')
    ),
    ADD CONSTRAINT commission_archived_at_in_range CHECK (
        archived_at IS NULL
        OR (archived_at >= '0001-01-01 00:00:00+00' AND archived_at <= '9999-12-31 23:59:59.999999+00')
    ),
    ADD CONSTRAINT commission_disputed_at_in_range CHECK (
        disputed_at IS NULL
        OR (disputed_at >= '0001-01-01 00:00:00+00' AND disputed_at <= '9999-12-31 23:59:59.999999+00')
    );
