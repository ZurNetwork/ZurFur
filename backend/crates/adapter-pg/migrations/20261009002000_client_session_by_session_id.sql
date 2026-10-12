-- A failed sign-in's cleanup deletes every `client_session` row under one
-- `session_id`. The table is keyed `PRIMARY KEY (account_did, session_id)`, and a
-- composite btree cannot range-seek on its trailing column (Postgres 16 here,
-- per docker-compose; B-tree skip scan is 18+), so without this index each
-- cleanup would scan every stored OAuth session. Plain, not unique: the key
-- shape is the OAuth library's.
CREATE INDEX client_session_by_session_id ON atproto_oauth.client_session (session_id);
