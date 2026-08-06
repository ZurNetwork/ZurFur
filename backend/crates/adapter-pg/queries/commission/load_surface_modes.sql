-- The commission's widened surface modes — the second term of the min
-- (ZMVP-166). SPARSE by design: a surface nobody widened has NO ROW, and an
-- absent row means Total (the closed door said by saying nothing), so this
-- returning fewer rows than there are declared surfaces is the normal case.
SELECT surface, mode FROM commission_surface_mode
WHERE commission_id = $1
ORDER BY surface
