-- Issue (or re-issue) one view grant. At most one key per (commission, grantee):
-- re-granting REPLACES the level rather than adding a second row — "issuing anew"
-- (Ownership Separation DD 29130754 Decision 5). `grantee` is the holder's DID
-- since the actor re-key (DD 57081857).
INSERT INTO commission_view_grant (commission_id, grantee, level)
VALUES ($1, $2, $3)
ON CONFLICT (commission_id, grantee)
DO UPDATE SET level = EXCLUDED.level
