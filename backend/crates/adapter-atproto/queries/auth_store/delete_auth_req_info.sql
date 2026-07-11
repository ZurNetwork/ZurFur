-- params: state
-- fetch: execute
DELETE FROM atproto_oauth.auth_request WHERE state = $1
