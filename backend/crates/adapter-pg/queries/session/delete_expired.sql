-- expiry_date receives the sweep instant ("now").
DELETE FROM tower_sessions.session WHERE expiry_date < $1