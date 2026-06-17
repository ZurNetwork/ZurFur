-- Server-side session store backing tower-sessions (see ZMVP-8). "Signed in"
-- means a row here holds the visitor's session; the cookie carries only the id,
-- so the session survives a browser reload.
--
-- Schema and column types mirror tower-sessions-sqlx-store's PostgresStore
-- defaults exactly (schema `tower_sessions`, table `session`). The composition
-- root therefore needs no builder configuration: `PostgresStore::new(pool)` finds
-- this table as-is. We own the schema here, so the store must NOT also call
-- `.migrate()` at boot — though `IF NOT EXISTS` keeps it harmless if it does.
CREATE SCHEMA IF NOT EXISTS tower_sessions;

CREATE TABLE IF NOT EXISTS tower_sessions.session (
    id          text        PRIMARY KEY NOT NULL,
    data        bytea       NOT NULL,
    expiry_date timestamptz NOT NULL
);
