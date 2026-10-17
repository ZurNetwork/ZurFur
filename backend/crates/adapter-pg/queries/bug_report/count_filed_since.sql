-- How many reports one reporter filed at or after `received_at`; a report
-- filed exactly then counts. Run inside the filing unit, after
-- `lock_reporter`.
SELECT count(*)
FROM bug_report
WHERE reporter_id = $1 AND received_at >= $2
