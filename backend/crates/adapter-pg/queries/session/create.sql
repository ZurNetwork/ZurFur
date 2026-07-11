INSERT INTO tower_sessions.session (id, data, expiry_date)
VALUES ($1, $2, $3)
ON CONFLICT (id) DO NOTHING
