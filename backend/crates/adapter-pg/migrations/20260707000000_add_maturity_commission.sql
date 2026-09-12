-- The commission's maturity posture: two nullable envelope columns,
-- `maturity` (the four-tier axis) and `graphic` (an orthogonal flag), always
-- set or cleared together. See NODE.md ("20260707000000_add_maturity_commission.sql")
-- for the full rationale.
ALTER TABLE commission
    ADD COLUMN maturity text,
    ADD COLUMN graphic boolean,
    ADD CONSTRAINT commission_maturity_graphic_together
        CHECK ((maturity IS NULL) = (graphic IS NULL));
