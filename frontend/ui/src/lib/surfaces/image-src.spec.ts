import { describe, expect, it } from 'vitest';
import { isSameOriginPath } from '../controls/link-href.ts';
import { isImageSrc } from './image-src.ts';

const accepted = ['/fixtures/a.svg', './fixtures/a.svg', '../../fixtures/a.svg', '/x?y=1#z'];

const refused = [
	'https://x.example/a.png',
	'//x.example/a.png',
	'/\\x',
	'/\t/x',
	'/\n/x',
	'/a/%2e%2e//evil.example/x',
	'/avatar/x/%2e%2e/%2e%2e/logout',
	'./%2e%2e/x',
	'/a/%2E%2e/b',
	'/a/../b',
	'/a/./b',
	'/a//b',
	'/a/%2f/b',
	'/a/%5C/b',
	'/',
	'.',
	'..',
	'../..',
	'./',
	'data:image/png;base64,AAAA',
	'data:image/svg+xml,<svg onload=alert(1)>',
	'javascript:alert(1)',
	'blob:https://x.example/1',
	' /x',
	'a.png',
	''
];

describe('isImageSrc', () => {
	it.each(accepted)('accepts %j', (text) => {
		expect(isImageSrc(text)).toBe(true);
	});

	it.each(refused)('refuses %j', (text) => {
		expect(isImageSrc(text)).toBe(false);
	});

	it('agrees with the same-origin path check on every case', () => {
		const everything = [...accepted, ...refused];
		const disagreements = everything.filter((text) => isImageSrc(text) !== isSameOriginPath(text));
		expect(disagreements).toEqual([]);
	});
});
