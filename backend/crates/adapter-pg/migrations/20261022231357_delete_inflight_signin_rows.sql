-- Deletes, once, every stored atproto OAuth session and pending sign-in, and
-- nothing else: they are sealed in a format this build no longer opens. Why a
-- migration and why TRUNCATE: this crate's NODE.inf.json notes, under this file.
TRUNCATE atproto_oauth.client_session, atproto_oauth.auth_request;
