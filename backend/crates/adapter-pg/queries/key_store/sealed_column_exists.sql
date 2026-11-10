-- Whether the custody table has its sealed-envelope column yet, so a read can
-- run on the schema before the custody envelope.
SELECT EXISTS (
    SELECT 1
    FROM pg_attribute
    WHERE attrelid = to_regclass('public.account_keys')
      AND attname = 'sealed_keys'
      AND NOT attisdropped
) AS present
