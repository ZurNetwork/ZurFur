import { mockModeEnabled } from './api/zurfur-api-mock';

/**
 * Whether this run serves a Den the web can show: only the mock today (the
 * backend's dev profile joins it later). It fails closed — on the everyday
 * stack, against the live API, it is false, so the frame shows no sidebar,
 * no tree and no "My Den" links for a Den it can't serve.
 */
export function denServed(): boolean {
	return mockModeEnabled();
}
