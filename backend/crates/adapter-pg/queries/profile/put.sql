INSERT INTO profile_cache (did, handle, display_name, avatar_url, fetched_at)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (did) DO UPDATE SET
    handle       = EXCLUDED.handle,
    display_name = EXCLUDED.display_name,
    avatar_url   = EXCLUDED.avatar_url,
    fetched_at   = EXCLUDED.fetched_at
