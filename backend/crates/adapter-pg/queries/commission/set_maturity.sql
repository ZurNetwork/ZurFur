-- Rate a commission by compare-and-set: the row changes only while it still holds the rating
-- and Graphic flag the change was checked from (both NULL while unrated), so a stale change
-- matches no row. The schema's both-or-neither CHECK keeps the pair whole.
UPDATE commission AS held
SET maturity = $2, graphic = $3
WHERE held.id = $1
  AND held.maturity IS NOT DISTINCT FROM $4
  AND held.graphic IS NOT DISTINCT FROM $5
