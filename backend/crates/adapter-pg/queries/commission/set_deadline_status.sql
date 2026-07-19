UPDATE commission SET deadline_status = $2 WHERE id = $1 AND deadline_status IS DISTINCT FROM $2
