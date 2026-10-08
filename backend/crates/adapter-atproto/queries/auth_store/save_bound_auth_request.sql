-- Save an in-flight auth request together with the hash of the browser token
-- that binds it, in one statement.
INSERT INTO atproto_oauth.auth_request (state, data, browser_binding, created_at)
VALUES ($1, $2, $3, now())
ON CONFLICT (state) DO UPDATE
SET data = excluded.data, browser_binding = excluded.browser_binding, created_at = now()
