/** Whether {@link loadFullPage} left the page, or refused an address that isn't http(s). */
export type FullPageLoad = 'left' | 'refused';

/**
 * Leave for `location` (absolute, or relative to this page) by a full page load,
 * never through the client router. Only an `http:` or `https:` address is followed.
 */
export function loadFullPage(location: string): FullPageLoad {
	const url = URL.parse(location, window.location.href) ?? undefined;
	if (url === undefined || (url.protocol !== 'http:' && url.protocol !== 'https:'))
		return 'refused';
	window.location.assign(url.href);
	return 'left';
}
