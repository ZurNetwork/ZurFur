-- Save an in-flight auth request together with the hash of the browser token
-- that binds it, in one statement. A plain insert: a `state` already stored is
-- an error, never an overwrite, so nothing can re-arm a claimed request.
INSERT INTO atproto_oauth.auth_request (state, data, browser_binding, created_at)
VALUES ($1, $2, $3, now())
