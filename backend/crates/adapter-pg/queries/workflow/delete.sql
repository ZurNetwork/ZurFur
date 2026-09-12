-- Delete a board. Its columns go with it (ON DELETE CASCADE), and the cards on
-- them — but never the commissions themselves: a card is account-side
-- positioning, and the commission never knew it was there.
DELETE FROM workflow WHERE id = $1
