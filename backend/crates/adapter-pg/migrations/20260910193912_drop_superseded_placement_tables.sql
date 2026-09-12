-- Drops the superseded account-level placement rails: a commission's
-- position is now a card in a workflow column, not a separate placement log.
-- See NODE.md ("20260910193912_drop_superseded_placement_tables.sql") for the
-- full rationale.

DROP TABLE commission_current_placement;
DROP TABLE commission_placement;
