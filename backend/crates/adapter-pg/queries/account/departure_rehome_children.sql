-- parent receives the departing member's own parent — their invitees are
-- re-homed onto it (NULL when the departing member was a root); m_parent
-- selects the departing member as the children's current parent.
UPDATE account_members AS m SET parent = $1 WHERE m.account_id = $2 AND m.parent = $3
