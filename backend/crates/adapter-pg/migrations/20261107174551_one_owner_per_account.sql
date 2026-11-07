-- An Account has at most one Owner membership. An Account already holding two stops the
-- migration, naming how many Accounts do, never an Account or a User; no row is written or
-- repaired. How to find the Accounts: this crate's NODE.inf.json notes, under this file.

-- No membership is written while the check runs and the index builds, so none slips in
-- between them. Reads go on.
LOCK TABLE account_members IN SHARE MODE;

-- Counts the Accounts the index would refuse: the predicate is the index's own, grouped by
-- Account. The refusal names the count alone.
DO $$
DECLARE
    breaking bigint;
BEGIN
    SELECT count(*) INTO breaking FROM (
        SELECT 1 FROM account_members m
        WHERE m.role = 'owner'
        GROUP BY m.account_id
        HAVING count(*) > 1
    ) doubled;
    IF breaking > 0 THEN
        RAISE EXCEPTION 'the one-owner index stops: % accounts have more than one owner', breaking;
    END IF;
END
$$;

CREATE UNIQUE INDEX account_members_one_owner ON account_members (account_id) WHERE role = 'owner';
