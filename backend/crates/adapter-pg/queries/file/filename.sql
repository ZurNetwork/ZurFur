-- The filename stored under one key, without its bytes.
SELECT filename
FROM file_blob
WHERE key = $1
