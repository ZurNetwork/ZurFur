-- params: id, data, expiry_date
-- fetch: execute
-- timestamptz: time
INSERT INTO tower_sessions.session (id, data, expiry_date)
VALUES ($1, $2, $3)
ON CONFLICT (id) DO UPDATE
SET data = excluded.data, expiry_date = excluded.expiry_date
