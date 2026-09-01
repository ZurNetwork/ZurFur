-- Drop every card edge of one column — the first half of the wholesale rewrite
-- `set_commissions` performs. Paired with `add_card.sql` inside one unit of work,
-- so the board is never observed empty.
DELETE FROM workflow_column_commission WHERE column_id = $1
