-- $2: lifetime_seconds
-- The browser-token hash of the auth request saved under `state`, if the request
-- is younger than `lifetime_seconds`; a request without a binding, or one a
-- callback has claimed, yields NULL.
SELECT browser_binding FROM atproto_oauth.auth_request
WHERE state = $1 AND created_at > now() - make_interval(secs => $2::integer)
