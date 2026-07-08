-- The commission's maturity posture (ZMVP-31; Maturity Vocabulary DD
-- DESIGN/29982722): two nullable envelope columns on the commission row —
-- like deadline, NOT a tree node (the Surfaces DD pins maturity's *render*
-- tier, Presentation, not a storage node).
--
-- maturity  The four-tier axis token ('safe' | 'suggestive' | 'nudity' |
--           'adult') — the domain enum owns the vocabulary and validates it
--           server-side on both write and read (text, not a pg enum, matching
--           lifecycle/visibility).
-- graphic   The orthogonal Graphic flag (gore is not a sexual-maturity
--           question — DD Decision 2), meaningful only alongside a rating.
--
-- Both start NULL for every commission, existing and new: a commission is
-- born unrated BY INVARIANT — birth commissions are Private (root Total), so
-- nobody outside can see anything and no rating is needed yet. The rating
-- becomes REQUIRED at the widening gate (ZMVP-74): "cannot be published
-- without a maturity value" maps to "cannot widen to non-participants
-- unrated" for this never-published Class B surface. No backfill is needed —
-- unrated is the correct state for every pre-existing (all Private-born)
-- commission.
--
-- The CHECK makes a half-set posture unrepresentable: rated = both columns,
-- unrated = neither (a graphic flag with no rating means nothing).
ALTER TABLE commission
    ADD COLUMN maturity text,
    ADD COLUMN graphic boolean,
    ADD CONSTRAINT commission_maturity_graphic_together
        CHECK ((maturity IS NULL) = (graphic IS NULL));
