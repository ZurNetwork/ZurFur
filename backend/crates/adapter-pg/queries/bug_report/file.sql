-- File one bug report. Written once and never changed. `error_status` is a
-- smallint column, bound as an integer (the generator maps no smallint).
INSERT INTO bug_report (
    id, reporter_id, description,
    page, page_kind, web_version, browser,
    window_width, window_height,
    reduced_motion, more_contrast, forced_colors,
    error_status, api_version, received_at
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13::integer, $14, $15)
