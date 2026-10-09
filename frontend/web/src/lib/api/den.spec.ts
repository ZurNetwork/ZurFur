import { describe, expect, it } from 'vitest';
import { isDenLink, isDenPageData } from './den';

/** A well-formed den page answer, as plain JSON (what a preload hands back). */
function pageData(entryHref: unknown = '/den/commissions/c1'): unknown {
	return {
		kind: 'denPage',
		rootHref: '/den',
		includeDeleted: false,
		continued: false,
		title: 'My Den · Zurfur',
		ancestors: [{ folder: '/den', entries: [], more: undefined }],
		trail: [{ step: 'root', href: '/den' }],
		outcome: {
			outcome: 'page',
			flagLinks: { on: '/den/commissions?includeDeleted=true', off: '/den/commissions' },
			page: {
				view: 'open',
				crumbs: [{ href: '/den', name: 'Alice' }],
				node: {
					view: 'open',
					href: '/den/commissions',
					name: 'commissions',
					type: 'user.commissions',
					kind: 'directory',
					mounted: false,
					ownLevel: 'private',
					contentNotShown: false,
					removed: undefined
				},
				body: {
					body: 'listing',
					more: undefined,
					entries: [
						{
							view: 'open',
							href: entryHref,
							name: 'Untitled',
							type: 'commission',
							kind: 'directory',
							mounted: true,
							ownLevel: 'private',
							contentNotShown: false,
							removed: undefined
						}
					]
				}
			}
		}
	};
}

describe('isDenPageData', () => {
	it('accepts a well-formed answer', () => {
		expect(isDenPageData(pageData())).toBe(true);
	});

	it.each([
		['an off-site link', '//evil.example/x'],
		['an absolute URL', 'https://evil.example/den'],
		['a link outside My Den', '/accounts'],
		['a prefix look-alike', '/dent/x'],
		['a backslash', '/den/a\\b'],
		['a dot-dot that resolves out of the Den', '/den/../accounts'],
		['an encoded dot-dot that resolves out of the Den', '/den/%2e%2e/accounts'],
		['a number', 42]
	])('refuses an answer whose entry link is %s', (_, href) => {
		expect(isDenPageData(pageData(href))).toBe(false);
	});

	it('refuses something that only claims to be a den page', () => {
		expect(isDenPageData({ kind: 'denPage' })).toBe(false);
		expect(isDenPageData(undefined)).toBe(false);
	});
});

describe('isDenLink', () => {
	it('takes /den, /den/… and /den?…', () => {
		expect([isDenLink('/den'), isDenLink('/den/a'), isDenLink('/den?includeDeleted=true')]).toEqual(
			[true, true, true]
		);
	});

	it.each(['/den/../accounts', '/den/%2e%2e/accounts', '/den/./x', '/den/a/../../accounts'])(
		'refuses %s, which URL resolution would move',
		(link) => {
			expect(isDenLink(link)).toBe(false);
		}
	);
});
