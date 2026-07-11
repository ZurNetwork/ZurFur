-- params: now
-- fetch: execute
-- timestamptz: time
DELETE FROM tower_sessions.session WHERE expiry_date < $1
