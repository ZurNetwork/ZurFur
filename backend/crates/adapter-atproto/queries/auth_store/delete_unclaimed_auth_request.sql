-- Delete the auth request saved under `state` unless a callback has claimed it
-- (cleared its binding): a refused callback's cleanup, which never removes a
-- claimed request, whatever it read before or however old the request is.
DELETE FROM atproto_oauth.auth_request
WHERE state = $1 AND browser_binding IS NOT NULL
