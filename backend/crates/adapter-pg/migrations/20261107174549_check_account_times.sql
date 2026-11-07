-- An Account's three times lie in the years 1 to 9999 UTC, which also refuses an
-- infinite time. A row already breaking a rule stops the migration, naming the column
-- and how many rows break it, never an Account; no row is written or repaired and the
-- table is not rewritten. How to find the breaking rows: this crate's NODE.inf.json
-- notes, under this file.

-- Nothing reads or writes an Account while the checks and the constraints land, so no
-- row slips in between them and no lock is upgraded.
LOCK TABLE accounts IN ACCESS EXCLUSIVE MODE;

-- Each rule counts the rows its constraint would refuse: the predicate is the
-- constraint's own expression, negated. Each refusal names a column and a count alone.
DO $$
DECLARE
    breaking bigint;
BEGIN
    SELECT count(*) INTO breaking FROM accounts a
    WHERE NOT (a.created_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00');
    IF breaking > 0 THEN
        RAISE EXCEPTION 'the account time checks stop: % accounts have a created_at outside years 1 to 9999', breaking;
    END IF;

    SELECT count(*) INTO breaking FROM accounts a
    WHERE NOT (a.updated_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00');
    IF breaking > 0 THEN
        RAISE EXCEPTION 'the account time checks stop: % accounts have an updated_at outside years 1 to 9999', breaking;
    END IF;

    SELECT count(*) INTO breaking FROM accounts a
    WHERE NOT (a.deleted_at IS NULL
               OR a.deleted_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00');
    IF breaking > 0 THEN
        RAISE EXCEPTION 'the account time checks stop: % accounts have a deleted_at outside years 1 to 9999', breaking;
    END IF;
END
$$;

-- Every constraint in one statement, so the table is scanned once to validate them.
-- Each bound carries its zone, so the session's time zone never moves it.
ALTER TABLE accounts
    ADD CONSTRAINT accounts_created_at_in_range CHECK (
        created_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00'
    ),
    ADD CONSTRAINT accounts_updated_at_in_range CHECK (
        updated_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00'
    ),
    ADD CONSTRAINT accounts_deleted_at_in_range CHECK (
        deleted_at IS NULL
        OR deleted_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00'
    );
