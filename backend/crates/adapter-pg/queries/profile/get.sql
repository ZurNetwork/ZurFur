SELECT did, handle, display_name, avatar_url
FROM profile_cache
WHERE did = $1 AND fetched_at > $2
