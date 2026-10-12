-- $3: lifetime_seconds
-- Consume the browser binding of the auth request saved under `state` if it is
-- still `browser_binding` and the request is younger than `lifetime_seconds`:
-- the one callback whose statement clears it claims the sign-in. A racing
-- statement waits on the row lock, re-checks the cleared row and claims nothing.
UPDATE atproto_oauth.auth_request
SET browser_binding = NULL
WHERE state = $1
  AND browser_binding = $2
  AND created_at > now() - make_interval(secs => $3::integer)
