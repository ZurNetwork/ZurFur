-- params: id, status?
-- fetch: execute
UPDATE commission SET deadline_status = $2 WHERE id = $1
