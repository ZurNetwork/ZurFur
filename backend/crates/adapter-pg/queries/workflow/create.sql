-- Mint one board for an account. The id and the closed-door default visibility
-- are the domain's (`Workflow::new`); this only records them.
INSERT INTO workflow (id, account_id, name, visibility)
VALUES ($1, $2, $3, $4)
