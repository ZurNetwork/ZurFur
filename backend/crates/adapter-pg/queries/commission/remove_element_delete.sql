-- Remove one element (ZMVP-166). Scoped by commission_id as well as the unique
-- id, so the statement is self-contained rather than leaning on id uniqueness
-- (PR #109 review). Elements are leaves — nothing is orphaned — but whatever
-- shares the element's identity (a Slot or Seat satellite, and a seat's pending
-- invitations) leaves with it via ON DELETE CASCADE.
DELETE FROM commission_element WHERE id = $1 AND commission_id = $2
