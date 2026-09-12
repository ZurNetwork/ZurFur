-- Issue (or re-issue) one view grant. At most one key per (commission, grantee):
-- re-granting REPLACES the level rather than adding a second row — "issuing anew".
-- `grantee` is the holder's DID: actors are addressed by DID everywhere, so
-- this column stores a DID rather than a surrogate id.
INSERT INTO commission_view_grant (commission_id, grantee, level)
VALUES ($1, $2, $3)
ON CONFLICT (commission_id, grantee)
DO UPDATE SET level = EXCLUDED.level
