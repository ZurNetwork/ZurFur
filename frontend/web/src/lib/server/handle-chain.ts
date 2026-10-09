import type { Handle, RequestEvent, ResolveOptions } from '@sveltejs/kit';

/**
 * `handles` run as one `handle` hook, the first outermost: each one's
 * `resolve` runs the next, and the last one's runs SvelteKit's own. A plain
 * function rather than SvelteKit's `sequence`, which needs the live request
 * store and so can't run in a unit test.
 */
export function handleChain(...handles: readonly Handle[]): Handle {
	return ({ event, resolve }) => {
		const run = (
			index: number,
			current: RequestEvent,
			options: ResolveOptions | undefined
		): ReturnType<Handle> => {
			const handle = handles[index];
			if (handle === undefined) return resolve(current, options);
			return handle({
				event: current,
				resolve: (next, nextOptions) => run(index + 1, next, nextOptions ?? options)
			});
		};
		return run(0, event, undefined);
	};
}
