-- The first time an Account reached each fact that keeps it from a hard delete: one
-- row per Account and kind, and the first time wins. `member`, a member besides its
-- founder, is the only kind known. Rows are only ever inserted: an update, a delete
-- or a truncate is refused. An Account holding a fact cannot be removed, since the
-- foreign key does not cascade. Every Account already holding the member fact gets
-- its row here. Why its times are what they are: this crate's NODE.inf.json notes,
-- under this file.

-- No membership or invitation is written while the facts are read from them, so no
-- member slips in between the read and the table's first writer.
LOCK TABLE account_members, account_invitations IN SHARE MODE;

CREATE TABLE account_fact (
    account_id text        NOT NULL,
    kind       text        NOT NULL,
    reached_at timestamptz NOT NULL,
    PRIMARY KEY (account_id, kind),
    CONSTRAINT account_fact_account_id_fkey FOREIGN KEY (account_id) REFERENCES accounts (id),
    CONSTRAINT account_fact_kind_known CHECK (kind IN ('member')),
    CONSTRAINT account_fact_reached_at_in_range CHECK (
        reached_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00'
    )
);

-- A fact, once reached, stays reached: every change but an insert is refused. The
-- message names the table alone.
CREATE FUNCTION account_fact_refuse_change() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'account_fact is insert-only: a fact is never changed or removed';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER account_fact_insert_only
    BEFORE UPDATE OR DELETE ON account_fact
    FOR EACH ROW EXECUTE FUNCTION account_fact_refuse_change();

CREATE TRIGGER account_fact_never_truncated
    BEFORE TRUNCATE ON account_fact
    FOR EACH STATEMENT EXECUTE FUNCTION account_fact_refuse_change();

-- An accepted invitation's time becomes a fact's time, so one outside the years 1 to
-- 9999 stops the backfill, named by its count alone, before anything is written.
DO $$
DECLARE
    breaking bigint;
BEGIN
    SELECT count(*) INTO breaking FROM account_invitations i
    WHERE lower(i.state) = 'accepted'
      AND NOT (i.updated_at BETWEEN '0001-01-01 00:00:00+00' AND '9999-12-31 23:59:59.999999+00');
    IF breaking > 0 THEN
        RAISE EXCEPTION 'the account fact backfill stops: % accepted invitations have an acceptance time outside years 1 to 9999', breaking;
    END IF;
END
$$;

-- An Account holds the member fact when it has an accepted invitation, two or more
-- members, or a member who is not an Owner; a state or role token matches in any
-- letter case. It is dated by its earliest accepted invitation; with none accepted,
-- by this migration's time, when it was recorded: when it was reached is unknown.
INSERT INTO account_fact (account_id, kind, reached_at)
SELECT a.id,
       'member',
       COALESCE(
           (SELECT min(i.updated_at) FROM account_invitations i
            WHERE i.account_id = a.id AND lower(i.state) = 'accepted'),
           now()
       )
FROM accounts a
WHERE EXISTS (SELECT 1 FROM account_invitations i
              WHERE i.account_id = a.id AND lower(i.state) = 'accepted')
   OR (SELECT count(*) FROM account_members m WHERE m.account_id = a.id) >= 2
   OR EXISTS (SELECT 1 FROM account_members m
              WHERE m.account_id = a.id AND lower(m.role) <> 'owner');
