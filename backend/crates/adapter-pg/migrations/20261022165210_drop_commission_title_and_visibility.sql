-- A commission's name and level live on its node alone: the commission's own
-- title and visibility columns go, with the trigger that copied a node's name
-- onto the title, and node_id becomes NOT NULL. First a check proves every
-- commission has its own node and holds the same title and visibility as its
-- node's name and level; any difference stops the migration, naming the
-- commission's id alone. No row is written and the table is not rewritten:
-- PostgreSQL's DROP COLUMN only hides a column. The old values stay in the
-- commission table's files, where no query or pg_dump returns them but a
-- superuser reading raw pages can, until each row is updated and vacuumed or
-- the table is rewritten. Old row versions, and the dropped columns'
-- statistics rows in pg_statistic, keep copies until vacuumed. Why the file
-- is shaped this way: this crate's NODE.inf.json notes, under this file.

-- No commission or node is written while the check and the drop run.
LOCK TABLE commission, node IN SHARE ROW EXCLUSIVE MODE;

-- Every commission has its node: one that exists, a top-level `commission`
-- node whose segment is the commission's id. The commission's title and
-- visibility are that node's name and level. Each refusal names an id alone.
-- Nothing is repaired.
DO $$
DECLARE
    offender uuid;
BEGIN
    SELECT c.id INTO offender FROM commission c
    LEFT JOIN node n ON n.id = c.node_id
        AND n.type = 'commission' AND n.parent IS NULL AND n.segment = c.id::text
    WHERE n.id IS NULL
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission column drop stops: commission % has no node of its own', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    JOIN node n ON n.id = c.node_id
    WHERE c.title IS DISTINCT FROM n.name
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission column drop stops: commission % has a title its node''s name differs from', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    JOIN node n ON n.id = c.node_id
    WHERE c.visibility IS DISTINCT FROM n.level
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission column drop stops: commission % has a visibility its node''s level differs from', offender;
    END IF;
END
$$;

-- The title the trigger kept equal to its node's name goes, so the trigger
-- goes first.
DROP TRIGGER commission_columns_follow_their_node ON node;
DROP FUNCTION commission_columns_follow_their_node();

-- Every row has its own node, so node_id holds it from here on and the check
-- that stood in for that goes. No CASCADE: whatever PostgreSQL records as
-- depending on a dropped column, such as a view or a column trigger, stops the
-- migration instead of being dropped with it. A function whose body is a
-- string records no such dependency: it doesn't stop the drop, and fails only
-- when called. The two columns carry no index or constraint of their own.
ALTER TABLE commission
    ALTER COLUMN node_id SET NOT NULL,
    DROP CONSTRAINT commission_has_its_node,
    DROP COLUMN title,
    DROP COLUMN visibility;
