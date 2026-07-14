-- params: id, commission_id, kind, prompt?, link?
-- fetch: execute
INSERT INTO commission_seat (id, commission_id, kind, prompt, link)
VALUES ($1, $2, $3, $4, $5)
