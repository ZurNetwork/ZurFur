INSERT INTO file_blob (key, filename, content_type, byte_size, bytes, created_at)
VALUES ($1, $2, $3, $4, $5, now())
ON CONFLICT (key) DO UPDATE
  SET filename = EXCLUDED.filename,
      content_type = EXCLUDED.content_type,
      byte_size = EXCLUDED.byte_size,
      bytes = EXCLUDED.bytes
