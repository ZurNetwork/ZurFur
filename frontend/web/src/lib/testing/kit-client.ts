/**
 * A stand-in for SvelteKit's client runtime, for specs that submit a form
 * through the real `enhance`. No app starts under test, so the real module
 * has no decoders to read an action's data with; this one has none to need,
 * and records every navigation, invalidation and applied result instead of
 * doing it. A spec mocks the runtime with it (the path is the spec's own
 * relative path to kit's `src/runtime/client/client.js`):
 *
 *     vi.mock('<…>/@sveltejs/kit/src/runtime/client/client.js', () => import('$lib/testing/kit-client'));
 *
 * `$app/navigation` and `$app/forms` then reach these functions, both from
 * the spec's code and from `enhance` itself.
 */

/** What the page asked SvelteKit for. */
export const kitCalls = {
	gotos: [] as string[],
	invalidations: 0,
	applied: [] as unknown[]
};

/** Forget every recorded call. */
export function resetKitCalls(): void {
	kitCalls.gotos.length = 0;
	kitCalls.invalidations = 0;
	kitCalls.applied.length = 0;
}

/** A flat record of strings as devalue writes it: how SvelteKit sends an action's data. */
export function devalued(record: Readonly<Record<string, string>>): string {
	const keys = Object.keys(record);
	const indexes = Object.fromEntries(keys.map((key, index) => [key, index + 1]));
	return JSON.stringify([indexes, ...keys.map((key) => record[key])]);
}

/** The started app: no transport decoders. */
export const app = { decoders: {} };

export const stores = {};

export function get_navigation_intent(): Promise<undefined> {
	return Promise.resolve(undefined);
}

export function applyAction(result: unknown): Promise<void> {
	kitCalls.applied.push(result);
	return Promise.resolve();
}

export function goto(location: string | URL): Promise<void> {
	kitCalls.gotos.push(String(location));
	return Promise.resolve();
}

function invalidated(): Promise<void> {
	kitCalls.invalidations += 1;
	return Promise.resolve();
}

export const invalidate = invalidated;
export const invalidateAll = invalidated;
export const refreshAll = invalidated;

function nothing(): void {
	return undefined;
}

export const afterNavigate = nothing;
export const beforeNavigate = nothing;
export const onNavigate = nothing;
export const disableScrollHandling = nothing;
export const pushState = nothing;
export const replaceState = nothing;

export function preloadData(): Promise<{ type: 'loaded'; status: number; data: object }> {
	return Promise.resolve({ type: 'loaded', status: 200, data: {} });
}

export function preloadCode(): Promise<void> {
	return Promise.resolve();
}
