-- Every report received at or after `received_at`, or every report when it is
-- NULL, oldest first, ties broken by id. `error_status` is a smallint
-- column, read as an integer (the generator maps no smallint).
SELECT id, reporter_id, description,
       page, page_kind, web_version, browser,
       window_width, window_height,
       reduced_motion, more_contrast, forced_colors,
       error_status::integer AS error_status, api_version, received_at
FROM bug_report
WHERE $1::timestamptz IS NULL OR received_at >= $1
ORDER BY received_at, id
