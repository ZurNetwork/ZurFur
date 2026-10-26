-- Whether the custody table exists, so a read can run before any migration.
SELECT EXISTS (
    SELECT 1 WHERE to_regclass('public.account_keys') IS NOT NULL
) AS present
