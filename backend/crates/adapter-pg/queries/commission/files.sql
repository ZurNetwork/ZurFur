-- Every file entry of one commission, key and upload time only (never the
-- uploader), ordered by key (UUIDv7, so upload order). Empty for a commission
-- with none and for an unknown one.
SELECT id, created_at
FROM commission_file
WHERE commission_id = $1
ORDER BY id
