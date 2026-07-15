-- params: id, kind, state
-- fetch: execute
INSERT INTO actor_identity (id, kind, state)
VALUES ($1, $2, $3)
