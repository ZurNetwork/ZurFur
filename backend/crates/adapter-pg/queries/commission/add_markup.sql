-- One annotation's canonical row. Written on the open transaction
-- alongside the markup_added changelog entry it accompanies, so the geometry and
-- its timeline fact land together. The (file_id, commission_id) composite foreign
-- key makes a markup on another commission's file unrepresentable, so no guard is
-- needed here beyond the caller's existence check.
INSERT INTO commission_markup
    (id, commission_id, file_id, added_by, shape, text, created_at)
VALUES ($1, $2, $3, $4, $5, $6, $7)
