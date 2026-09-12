-- Workflow storage: the account-side positioning rail, and the FIRST tables to
-- back it — `PgWorkflowStore`/`PgColumnStore` and their write twins were empty
-- stubs before this. **Accounts own positioning** — a workflow belongs to
-- exactly one account, and placement IS board membership, so one commission
-- may sit on N accounts' boards at once with no conflict. See NODE.md
-- ("20260910185856_create_workflow_and_columns.sql") for the full rationale.

-- One board. `visibility` is the board's own default posture; a column may set
-- its own apart from it, and neither confers any privacy on the cards — what an
-- outsider sees is each commission's own per-viewer projection (DESIGN/Workflow,
-- "a list holds no privacy of its own").
--
-- account_id     text, because an Account is addressed by its DID and nothing
--                else. CASCADE: a board is
--                positioning state, so it dies with the account that owned it.
-- visibility     The `private` | `listed` | `public` token, the same closed
--                vocabulary `commission.visibility` stores.
CREATE TABLE workflow (
    id         uuid PRIMARY KEY,
    account_id text NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    name       text NOT NULL,
    visibility text NOT NULL
);

-- The one read: an account's boards.
CREATE INDEX workflow_account ON workflow (account_id);

-- A board's Lists. Named `workflow_column` and not `column`, which is a reserved
-- word — the singular-table convention with the reserved name spelled out rather
-- than quoted at every call site.
--
-- position       The base-62 fractional key `domain::elements::workflow::Position`
--                mints (`Position::between`): inserting a column mints a key
--                BETWEEN its neighbours and nothing is ever renumbered. The
--                domain compares it BYTEWISE, so the column is `COLLATE "C"` —
--                under any other collation ('a' vs 'B') the store would order
--                differently from the code that produced the keys, which is a
--                silent board scramble rather than an error.
-- name           Unique per board: `Workflow::loaded` refuses a duplicate on the
--                way in, and the constraint keeps the store from being the place
--                that breaks it.
CREATE TABLE workflow_column (
    id          uuid PRIMARY KEY,
    workflow_id uuid NOT NULL REFERENCES workflow (id) ON DELETE CASCADE,
    name        text NOT NULL,
    visibility  text NOT NULL,
    position    text COLLATE "C" NOT NULL,
    UNIQUE (workflow_id, name)
);

-- The one read: a board's columns in board order.
CREATE INDEX workflow_column_ordered ON workflow_column (workflow_id, position);

-- The card edge — a commission's placement on one board, in one list, at one
-- spot.
--
-- position       An INTEGER index, not a fractional key, because the domain's
--                `Column.commissions` is a plain ordered `Vec<CommissionId>`
--                with no per-card key: `ColumnWrites::set_commissions` hands
--                over the whole list and the adapter rewrites it wholesale, so
--                there is no insert-between to keep cheap. (Integer position is
--                also what survives from the earlier tree-storage design.)
-- PRIMARY KEY    (column_id, commission_id) — a card enters a column at most
--                once, matching `Column::loaded`'s refusal of a card listed
--                twice.
-- UNIQUE         (column_id, position) DEFERRABLE, because a wholesale rewrite
--                legitimately passes through duplicate indexes mid-statement;
--                the constraint is checked once at COMMIT, when the list must be
--                a clean 0..n again.
CREATE TABLE workflow_column_commission (
    column_id     uuid NOT NULL REFERENCES workflow_column (id) ON DELETE CASCADE,
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    position      integer NOT NULL,
    PRIMARY KEY (column_id, commission_id),
    CONSTRAINT workflow_column_commission_position_unique
        UNIQUE (column_id, position) DEFERRABLE INITIALLY DEFERRED
);

-- The one read: a column's cards in board order.
CREATE INDEX workflow_column_commission_ordered
    ON workflow_column_commission (column_id, position);

-- The reverse read: which column of a given board holds a card
-- (`ColumnStore::find_column`, `CommissionStore::current_column_of_workflow`).
CREATE INDEX workflow_column_commission_by_commission
    ON workflow_column_commission (commission_id);
