-- `find`, with the commission row locked until the unit of work commits. FOR NO
-- KEY UPDATE is the weaker of the two row locks: it blocks a concurrent writer of
-- THIS commission while still letting other transactions insert children that
-- reference it (an element, a changelog entry), which a plain FOR UPDATE would
-- stall.
SELECT title, owner_id, lifecycle, visibility, deadline, maturity, graphic,
       direction_status, deadline_status, linked_channel, archived_at, created_at
FROM commission
WHERE id = $1
FOR NO KEY UPDATE
