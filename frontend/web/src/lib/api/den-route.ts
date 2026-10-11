import { resolve } from '$app/paths';
import type { ResolvedPathname } from '$app/types';

/**
 * My Den's root as an absolute path. Den links are built on it, not on
 * `resolve()`, because a server render resolves to relative paths and the
 * path builder must check an absolute one. (The app sets no base path.)
 */
export const DEN_ROOT = '/den';

/** My Den's root route for a plain link, through `resolve()`: the den page's rest parameter, empty. */
export function denRootPath(): ResolvedPathname {
	return resolve('/(session)/den/[...path]', { path: '' });
}
