-- params: id, maturity, graphic
-- fetch: execute
UPDATE commission SET maturity = $2, graphic = $3 WHERE id = $1
