-- Every commission gets its node: a commission made since the node table came
-- gets one, with its six entries, and every node's name is re-synced to its
-- commission's title. From here on a commission node has its commission, a new
-- or updated commission row has a node, and a node's name and level are copied
-- back onto its commission's title and visibility. Why the file is shaped this
-- way: this crate's NODE.json notes, under this file.

-- No commission or node is written while the checks and the fill run.
LOCK TABLE commission, node IN SHARE ROW EXCLUSIVE MODE;

-- At commit, a commission node has the commission that points at it. A node
-- deleted since is not checked.
CREATE FUNCTION node_has_its_thing() RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM node n WHERE n.id = NEW.id) THEN
        RETURN NULL;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM commission c WHERE c.node_id = NEW.id) THEN
        RAISE EXCEPTION 'node % has no commission: a commission node stands only with its commission',
            NEW.id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER node_has_its_thing
    AFTER INSERT ON node
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    WHEN (NEW.type = 'commission')
    EXECUTE FUNCTION node_has_its_thing();

-- Until the commission's own title and visibility columns go, a commission
-- node's name and level are copied onto them, where they differ.
CREATE FUNCTION commission_columns_follow_their_node() RETURNS trigger AS $$
BEGIN
    UPDATE commission c
    SET title = NEW.name, visibility = NEW.level
    WHERE c.node_id = NEW.id
      AND (c.title IS DISTINCT FROM NEW.name OR c.visibility IS DISTINCT FROM NEW.level);
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER commission_columns_follow_their_node
    AFTER UPDATE OF name, level ON node
    FOR EACH ROW
    WHEN (NEW.type = 'commission')
    EXECUTE FUNCTION commission_columns_follow_their_node();

-- A new or updated commission row has a node. Not validated: the rows the
-- fill below binds are the only ones without.
ALTER TABLE commission ADD CONSTRAINT commission_has_its_node CHECK (node_id IS NOT NULL) NOT VALID;

-- Everything the fill and the re-sync read must be readable before any row is
-- written. Each refusal names an id alone. Nothing is repaired.
DO $$
DECLARE
    offender uuid;
BEGIN
    SELECT c.id INTO offender FROM commission c
    WHERE c.visibility NOT IN ('private', 'listed', 'public')
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission node fill stops: commission % has an unreadable visibility', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE c.title = ''
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission node fill stops: commission % has an empty title', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    JOIN node n ON n.id = c.node_id
    WHERE c.visibility IS DISTINCT FROM n.level
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission node fill stops: commission % has a visibility its node''s level differs from', offender;
    END IF;

    SELECT n.id INTO offender FROM node n
    WHERE n.type = 'commission'
      AND n.parent IS NULL
      AND NOT EXISTS (SELECT 1 FROM commission c WHERE c.node_id = n.id)
    ORDER BY n.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission node fill stops: node % is a commission node no commission points at', offender;
    END IF;

    SELECT n.id INTO offender FROM node n
    CROSS JOIN (VALUES ('attachments'), ('products'), ('slots'), ('changelog'), ('participants'), ('brief'))
        AS entry (word)
    WHERE n.type = 'commission'
      AND NOT EXISTS (
          SELECT 1 FROM node e
          WHERE e.parent = n.id AND e.type = 'commission.' || entry.word
      )
    ORDER BY n.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the commission node fill stops: node % is missing one of its six entries', offender;
    END IF;
END
$$;

-- Each commission without a node gets one: its id as the segment, its
-- visibility as the level, its title as the name. Archived commissions too.
-- Only the nodes this statement inserts are bound.
WITH made AS (
    INSERT INTO node (id, parent, segment, type, level, name)
    SELECT gen_random_uuid(), NULL, c.id::text, 'commission', c.visibility, c.title
    FROM commission c
    WHERE c.node_id IS NULL
    RETURNING id, segment
)
UPDATE commission c
SET node_id = made.id
FROM made
WHERE made.segment = c.id::text;

-- The six entries, private, under each commission node holding none.
INSERT INTO node (id, parent, segment, type, level, name)
SELECT gen_random_uuid(), n.id, entry.word, 'commission.' || entry.word, 'private', entry.word
FROM commission c
JOIN node n ON n.id = c.node_id
CROSS JOIN (VALUES ('attachments'), ('products'), ('slots'), ('changelog'), ('participants'), ('brief'))
    AS entry (word)
WHERE NOT EXISTS (SELECT 1 FROM node e WHERE e.parent = n.id);

-- Each node's name follows its commission's title, renamed since its node was
-- made. Levels are not re-synced: a level that differs stopped the check above.
UPDATE node n
SET name = c.title
FROM commission c
WHERE c.node_id = n.id
  AND n.name IS DISTINCT FROM c.title;
