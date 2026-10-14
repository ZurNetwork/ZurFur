/** Leave for `location` by a full page load, never through the client router. */
export function loadFullPage(location: string): void {
	window.location.assign(location);
}
