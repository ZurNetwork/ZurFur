-- params: did, cid, op_type, prev?, operation, created_at
-- fetch: execute
INSERT INTO plc_operations (did, cid, "type", prev, operation, created_at)
VALUES ($1, $2, $3, $4, $5, $6)
