-- Record that the account reached the fact `kind` at `reached_at`. An account that
-- already holds the fact keeps its first time.
INSERT INTO account_fact (account_id, kind, reached_at)
VALUES ($1, $2, $3)
ON CONFLICT (account_id, kind) DO NOTHING
