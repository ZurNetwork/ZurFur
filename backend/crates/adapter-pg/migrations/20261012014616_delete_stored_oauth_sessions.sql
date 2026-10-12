-- Deletes, once, every stored atproto OAuth session and pending sign-in, and
-- nothing else: they were asked for with a scope that may write to the
-- visitor's repo, and sign-in asks for `atproto` alone from this migration on.
-- An older build can no longer start on a database that has run it. Why a
-- migration and why TRUNCATE: this crate's NODE.json notes, under this file.
TRUNCATE atproto_oauth.client_session, atproto_oauth.auth_request;
