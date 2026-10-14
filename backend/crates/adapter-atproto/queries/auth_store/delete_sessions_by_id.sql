-- Delete every session stored under `session_id`, whatever its account: the
-- cleanup after a sign-in's callback saved a session, whether it failed or not.
DELETE FROM atproto_oauth.client_session
WHERE session_id = $1
