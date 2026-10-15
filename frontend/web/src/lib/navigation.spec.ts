import { afterEach, describe, expect, it, vi } from 'vitest';
import { loadFullPage } from './navigation';

/** A window whose every full page load is recorded instead of followed. */
function stubWindow(): ReturnType<typeof vi.fn<(href: string) => void>> {
	const assign = vi.fn<(href: string) => void>();
	vi.stubGlobal('window', { location: { href: 'http://127.0.0.1:8180/login', assign } });
	return assign;
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('loadFullPage', () => {
	it.each([
		['/signin-callback?code=dev', 'http://127.0.0.1:8180/signin-callback?code=dev'],
		['https://pds.example/oauth/authorize?x=1', 'https://pds.example/oauth/authorize?x=1'],
		['http://127.0.0.1:8180/', 'http://127.0.0.1:8180/']
	])('follows %s', (location, href) => {
		const assign = stubWindow();

		expect(loadFullPage(location)).toBe('left');
		expect(assign.mock.calls).toEqual([[href]]);
	});

	it.each([
		'javascript:alert(1)',
		'data:text/html,<p>hi</p>',
		'file:///etc/passwd',
		'blob:http://127.0.0.1:8180/x',
		'http://[::1'
	])('refuses %s and does nothing', (location) => {
		const assign = stubWindow();

		expect(loadFullPage(location)).toBe('refused');
		expect(assign).not.toHaveBeenCalled();
	});
});
