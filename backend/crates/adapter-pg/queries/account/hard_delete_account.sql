-- Delete a live account's row. A deactivated account matches no row, and one holding
-- a fact is refused by the facts table's foreign key.
DELETE FROM accounts WHERE id = $1 AND deleted_at IS NULL
