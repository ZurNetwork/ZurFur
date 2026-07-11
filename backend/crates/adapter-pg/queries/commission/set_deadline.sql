-- params: id, deadline?
-- fetch: execute
UPDATE commission SET deadline = $2 WHERE id = $1
