-- params: state
-- fetch: optional
SELECT data FROM atproto_oauth.auth_request WHERE state = $1
