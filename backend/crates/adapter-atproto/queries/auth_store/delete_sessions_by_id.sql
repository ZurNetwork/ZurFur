-- Delete every session stored under `session_id`, whatever its account: the
-- cleanup for a sign-in that failed after its callback saved a session.
DELETE FROM atproto_oauth.client_session
WHERE session_id = $1
