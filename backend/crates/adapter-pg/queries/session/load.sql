-- expiry_date receives the load instant ("now"); expired sessions miss.
SELECT data FROM tower_sessions.session
WHERE id = $1 AND expiry_date > $2