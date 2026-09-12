-- One User by id. `users.id` IS the visitor's DID, so the projection carries
-- everything the caller needs and the actor_identity
-- join that used to recover the DID is gone.
SELECT u.id, u.created_at
FROM users u
WHERE u.id = $1
