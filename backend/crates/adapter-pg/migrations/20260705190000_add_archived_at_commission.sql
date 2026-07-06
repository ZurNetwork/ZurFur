-- ZMVP-68: the archive flag — NULL = active, a timestamp = when the owner
-- archived it. Soft path only (Deletion DD 3014657): the row and its facts
-- survive; active-view listings filter on `archived_at IS NULL`.
ALTER TABLE commission
ADD COLUMN archived_at timestamptz;
