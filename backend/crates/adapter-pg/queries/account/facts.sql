-- Every fact kind the account has reached, in token order.
SELECT kind FROM account_fact WHERE account_id = $1 ORDER BY kind
