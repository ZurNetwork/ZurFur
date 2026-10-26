import { describe, expect, it } from 'vitest';
import { isLinkHref, isSameOriginPath } from './link-href.ts';

const sameOriginPaths = [
	'/accounts',
	'/den/a?x=1#y',
	'./accounts',
	'../../den/x',
	'/a/b.c',
	'/a/..b',
	'/a/b?next=..',
	'/a/b?x=a//b'
];

const bareRoots = ['/', '.', './', '..', '../', '../..', '../../'];

const refused = [
	'/a/%2e%2e//evil.example/x',
	'/avatar/x/%2e%2e/%2e%2e/logout',
	'/a/../b',
	'/a/./b',
	'/a/.',
	'/a/%2E/b',
	'/a/.%2e/b',
	'/a/%2e./b',
	'./%2e/x',
	'../../x/../..',
	'/a//b',
	'/a/%2F/b',
	'/a%2fb',
	'/a%5Cb',
	'/a?x=%2f',
	'javascript:alert(1)',
	'data:text/html,x',
	'//evil.example',
	'/\\evil',
	'/\t/evil.example',
	'/\n/evil.example',
	'https://x',
	' /x',
	'.//evil',
	'..//evil',
	'accounts',
	''
];

/**
 * The relative forms SvelteKit's paths emit while rendering a page `depth`
 * folders below the root (`runtime/server/page/render.js` and
 * `runtime/app/paths/server.js`): `base` is `..` once per folder, or `.`, and
 * `resolve(id)` puts that in front of the route, which starts with `/`.
 */
function kitRelativeForms(depth: number, route: string): readonly string[] {
	const base = Array.from({ length: depth }, () => '..').join('/') || '.';
	return [base, `${base}/`, `${base}${route}`];
}

describe('isSameOriginPath', () => {
	it.each(sameOriginPaths)('accepts %j', (text) => {
		expect(isSameOriginPath(text)).toBe(true);
	});

	it.each(['/', './', '..', '../..'])('refuses the bare root %j', (text) => {
		expect(isSameOriginPath(text)).toBe(false);
	});

	it.each(refused)('refuses %j', (text) => {
		expect(isSameOriginPath(text)).toBe(false);
	});
});

describe('isLinkHref', () => {
	it.each([...sameOriginPaths, ...bareRoots])('accepts %j', (text) => {
		expect(isLinkHref(text)).toBe(true);
	});

	it.each(refused)('refuses %j', (text) => {
		expect(isLinkHref(text)).toBe(false);
	});

	it.each([0, 1, 2])('accepts the relative paths SvelteKit emits %i folders deep', (depth) => {
		const forms = kitRelativeForms(depth, '/den/commissions');
		expect(forms.filter((form) => !isLinkHref(form))).toEqual([]);
	});

	it('accepts the absolute paths SvelteKit emits with no base', () => {
		const forms = ['/', '/den/commissions', '/den/commissions/'];
		expect(forms.filter((form) => !isLinkHref(form))).toEqual([]);
	});

	it('mints only what it accepts, so every same-origin path is a link', () => {
		const everything = [...sameOriginPaths, ...bareRoots, ...refused];
		const paths = everything.filter((text) => isSameOriginPath(text));
		expect(paths.filter((text) => !isLinkHref(text))).toEqual([]);
	});
});
