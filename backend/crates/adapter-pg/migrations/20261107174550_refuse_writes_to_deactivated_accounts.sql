-- The store refuses every write into a deactivated Account, inside the writing
-- statement: a row trigger on each table that names an Account, directly or through
-- its board, raises `account_deactivated`. A write into a live Account holds the
-- Account row `FOR SHARE` until the unit ends, so a soft delete waits for it. Let
-- through on a deactivated Account: a member's departure (the membership's delete,
-- the re-homing of their invitees, the revoking of their pending offers) and the
-- decline of a pending offer. Let through everywhere: the deletes an Account's hard
-- delete cascades into, and the deletes of board columns and cards, which a
-- commission's own delete cascades into. The message names no Account.

CREATE FUNCTION refuse_write_to_deactivated_account() RETURNS trigger AS $$
DECLARE
    target text;
    deactivated timestamptz;
BEGIN
    IF TG_OP = 'DELETE' THEN
        target := OLD.account_id;
    ELSE
        target := NEW.account_id;
    END IF;

    SELECT a.deleted_at INTO deactivated FROM accounts a WHERE a.id = target FOR SHARE;

    -- No Account row: a delete is the Account's own hard delete cascading, and an
    -- insert or update is left to the foreign key, which refuses it.
    IF NOT FOUND OR deactivated IS NULL THEN
        IF TG_OP = 'DELETE' THEN
            RETURN OLD;
        END IF;
        RETURN NEW;
    END IF;

    -- A member's departure: the membership's delete, and the re-homing of the
    -- leaver's invitees, an update changing nothing but `parent`.
    IF TG_TABLE_NAME = 'account_members' THEN
        IF TG_OP = 'DELETE' THEN
            RETURN OLD;
        END IF;
        IF TG_OP = 'UPDATE' AND (to_jsonb(NEW) - 'parent') = (to_jsonb(OLD) - 'parent') THEN
            RETURN NEW;
        END IF;
    END IF;

    -- A decline, and a departure revoking the leaver's offers: a pending offer turned
    -- revoked, changing nothing but its state and `updated_at`.
    IF TG_TABLE_NAME = 'account_invitations'
       AND TG_OP = 'UPDATE'
       AND OLD.state = 'pending'
       AND NEW.state = 'revoked'
       AND (to_jsonb(NEW) - 'state' - 'updated_at') = (to_jsonb(OLD) - 'state' - 'updated_at') THEN
        RETURN NEW;
    END IF;

    RAISE EXCEPTION 'the account is deactivated'
        USING ERRCODE = 'check_violation', CONSTRAINT = 'account_deactivated';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER account_members_refuse_deactivated
    BEFORE INSERT OR UPDATE OR DELETE ON account_members
    FOR EACH ROW EXECUTE FUNCTION refuse_write_to_deactivated_account();

CREATE TRIGGER account_invitations_refuse_deactivated
    BEFORE INSERT OR UPDATE OR DELETE ON account_invitations
    FOR EACH ROW EXECUTE FUNCTION refuse_write_to_deactivated_account();

CREATE TRIGGER account_handle_changes_refuse_deactivated
    BEFORE INSERT OR UPDATE OR DELETE ON account_handle_changes
    FOR EACH ROW EXECUTE FUNCTION refuse_write_to_deactivated_account();

CREATE TRIGGER account_fact_refuse_deactivated
    BEFORE INSERT OR UPDATE OR DELETE ON account_fact
    FOR EACH ROW EXECUTE FUNCTION refuse_write_to_deactivated_account();

CREATE TRIGGER workflow_refuse_deactivated
    BEFORE INSERT OR UPDATE OR DELETE ON workflow
    FOR EACH ROW EXECUTE FUNCTION refuse_write_to_deactivated_account();

-- A column or a card names its Account through its board. Only inserts and updates
-- are refused: a commission's delete cascades into its cards on any board.
CREATE FUNCTION refuse_board_write_to_deactivated_account() RETURNS trigger AS $$
DECLARE
    board uuid;
    deactivated timestamptz;
BEGIN
    IF TG_TABLE_NAME = 'workflow_column' THEN
        board := NEW.workflow_id;
    ELSE
        SELECT c.workflow_id INTO board FROM workflow_column c WHERE c.id = NEW.column_id;
    END IF;

    SELECT a.deleted_at INTO deactivated
    FROM workflow w
    JOIN accounts a ON a.id = w.account_id
    WHERE w.id = board
    FOR SHARE OF a;

    -- No board or Account: left to the foreign key, which refuses it.
    IF NOT FOUND OR deactivated IS NULL THEN
        RETURN NEW;
    END IF;

    RAISE EXCEPTION 'the account is deactivated'
        USING ERRCODE = 'check_violation', CONSTRAINT = 'account_deactivated';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER workflow_column_refuse_deactivated
    BEFORE INSERT OR UPDATE ON workflow_column
    FOR EACH ROW EXECUTE FUNCTION refuse_board_write_to_deactivated_account();

CREATE TRIGGER workflow_column_commission_refuse_deactivated
    BEFORE INSERT OR UPDATE ON workflow_column_commission
    FOR EACH ROW EXECUTE FUNCTION refuse_board_write_to_deactivated_account();
