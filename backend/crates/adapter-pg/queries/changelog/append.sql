-- params: commission_id, kind, actor_id?, payload, note?, created_at
-- fetch: execute
INSERT INTO commission_changelog
    (commission_id, kind, actor_id, payload, note, created_at)
VALUES ($1, $2, $3, $4, $5, $6)
