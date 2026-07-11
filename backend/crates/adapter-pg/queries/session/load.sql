-- params: id, now
-- fetch: optional
-- timestamptz: time
SELECT data FROM tower_sessions.session
WHERE id = $1 AND expiry_date > $2
