-- Consume the browser binding of the auth request saved under `state` if it is
-- still `browser_binding`: the one callback whose statement clears it claims the
-- sign-in. A racing statement waits on the row lock, re-checks the cleared row
-- and claims nothing.
UPDATE atproto_oauth.auth_request
SET browser_binding = NULL
WHERE state = $1 AND browser_binding = $2
