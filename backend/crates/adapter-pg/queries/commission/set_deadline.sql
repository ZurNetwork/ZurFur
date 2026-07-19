UPDATE commission SET deadline = $2 WHERE id = $1 AND deadline IS DISTINCT FROM $2
