-- params: key
-- fetch: optional
-- row: FileBlobRow
SELECT filename, content_type, byte_size, bytes
FROM file_blob
WHERE key = $1
