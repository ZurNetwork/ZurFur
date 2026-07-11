-- params: account_did, session_id
-- fetch: execute
DELETE FROM atproto_oauth.client_session
WHERE account_did = $1 AND session_id = $2
