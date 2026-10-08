-- Binds each in-flight sign-in to the browser that started it. `browser_binding`
-- holds the SHA-256 of a random token handed to that browser in a short-lived
-- cookie; the callback must present the token, or the sign-in is refused. The
-- raw token is never stored. A request saved before this column existed has no
-- binding, so its callback fails closed.
ALTER TABLE atproto_oauth.auth_request
    ADD COLUMN browser_binding bytea;
