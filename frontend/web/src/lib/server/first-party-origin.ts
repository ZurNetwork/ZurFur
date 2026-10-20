/**
 * The site's own origin, read once from ZURFUR_PUBLIC_URL when the server
 * loads: the yardstick for every cross-site write and for any address the
 * web accepts as one of its own pages. Never the request's own address.
 */

import { dev } from '$app/environment';
import { env } from '$env/dynamic/private';
import { firstPartyOrigin, type FirstPartyOrigin } from '$lib/types/brand';

/**
 * The browser-visible origin a dev build assumes when ZURFUR_PUBLIC_URL is
 * unset — matches the Caddyfile, `.env.example` and the backend's dev config.
 */
export const DEV_PUBLIC_URL = 'http://127.0.0.1:8080';

/**
 * The run-time environment, where any variable may be unset. `svelte-kit sync`
 * types a variable it finds set as always present, so reads go through this view.
 */
function runtimeEnv(): Readonly<Record<string, string | undefined>> {
	return env;
}

/**
 * The configured origin. A dev build falls back to {@link DEV_PUBLIC_URL} only
 * when ZURFUR_PUBLIC_URL is unset; a malformed value, or an unset one outside
 * dev, leaves none.
 */
function configuredFirstParty(): FirstPartyOrigin | undefined {
	const raw = runtimeEnv().ZURFUR_PUBLIC_URL ?? (dev ? DEV_PUBLIC_URL : undefined);
	return raw === undefined ? undefined : firstPartyOrigin(raw);
}

/** The site's own origin, or `undefined` when none is configured (every write is then refused). */
export const firstParty: FirstPartyOrigin | undefined = configuredFirstParty();
