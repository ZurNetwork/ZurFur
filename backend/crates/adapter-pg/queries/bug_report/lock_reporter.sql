-- Lock one reporter's User row until the unit of work ends, so two units filing
-- for the same reporter count and insert one after the other. The rate limit's
-- count runs as its own statement after this one, so it sees what the previous
-- holder committed.
-- FOR NO KEY UPDATE serializes these units as FOR UPDATE would, without stalling child inserts.
SELECT u.id
FROM users u
WHERE u.id = $1
FOR NO KEY UPDATE
