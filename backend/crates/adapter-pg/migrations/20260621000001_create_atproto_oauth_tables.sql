-- Persistent home for atproto OAuth state, replacing jacquard's in-memory
-- MemoryAuthStore (see ZMVP-12). The store impl lives in adapter-atproto (it
-- speaks the jacquard `ClientAuthStore` trait); the schema lives here because
-- adapter-pg owns all DDL and migration machinery. Two row families mirror the
-- trait's two responsibilities.
--
-- These are app-owned, private-boundary rows: token sets and the DPoP private
-- key are secrets we hold on the user's behalf. They are stored as-is (JSON in
-- bytea), matching tower_sessions.session's plaintext precedent; envelope
-- encryption at rest is a separate, not-yet-pulled concern.
CREATE SCHEMA IF NOT EXISTS atproto_oauth;

-- Established OAuth sessions: access/refresh tokens, DPoP key + nonces, keyed by
-- account DID + session id. Persisting these lets the upstream grant and
-- jacquard's refresh machinery survive a restart or a move between replicas.
CREATE TABLE IF NOT EXISTS atproto_oauth.client_session (
    account_did text        NOT NULL,
    session_id  text        NOT NULL,
    data        bytea       NOT NULL,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (account_did, session_id)
);

-- In-flight authorization requests: PKCE verifier + DPoP key, keyed by the OAuth
-- `state`. Short-lived — written at /signin, read then deleted at the callback.
-- Persisting them lets /signin and /signin-callback be served by different
-- processes (the multi-replica case MemoryAuthStore could not cover).
CREATE TABLE IF NOT EXISTS atproto_oauth.auth_request (
    state      text        PRIMARY KEY NOT NULL,
    data       bytea       NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
