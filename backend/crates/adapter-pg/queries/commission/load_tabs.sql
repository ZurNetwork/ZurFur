-- Every tab of one commission — the first term of the effective-visibility min
-- (ZMVP-166). Ordered by the declared tab id for determinism.
SELECT id, tab, mode FROM commission_tab
WHERE commission_id = $1
ORDER BY tab
