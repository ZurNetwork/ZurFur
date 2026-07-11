-- params: commission_id
-- fetch: many
-- row: ChangelogRow
SELECT seq, kind, actor_id, payload, note, created_at
FROM commission_changelog
WHERE commission_id = $1
ORDER BY seq
