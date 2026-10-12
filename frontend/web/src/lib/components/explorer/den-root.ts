import { DEN_ROOT } from '$lib/api/den-route';
import { denHref, type DenHref } from '$lib/types/brand';

/**
 * My Den's root link before any Den page has said it (on an Accounts page,
 * say): the bare root, built from no input at all, so it needs no path
 * builder.
 */
export function denRootFallback(): DenHref {
	return denHref(DEN_ROOT);
}
