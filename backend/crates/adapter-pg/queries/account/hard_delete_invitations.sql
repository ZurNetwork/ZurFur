-- params: account_id
-- fetch: execute
DELETE FROM account_invitations WHERE account_id = $1
