import { describe, expect, it } from 'vitest';
import { MOTION_ATTRIBUTE, parseSkin, SKIN_ATTRIBUTE } from './environment';

describe('parseSkin', () => {
	it.each([
		['dark', 'dark'],
		['light', 'light']
	])('reads %s', (raw, expected) => {
		expect(parseSkin(raw)).toBe(expected);
	});

	it.each([
		['nothing', undefined],
		['empty text', ''],
		['upper case', 'LIGHT'],
		['mixed case', 'Dark'],
		['a trailing space', 'light '],
		['a leading space', ' dark'],
		['a newline', 'dark\n'],
		['an unknown skin', 'sepia'],
		['the object prototype key', '__proto__'],
		['the constructor key', 'constructor'],
		['an inherited method name', 'toString'],
		['a property of Object', 'hasOwnProperty']
	])('gives undefined for %s', (_label, raw) => {
		expect(parseSkin(raw)).toBeUndefined();
	});
});

describe('the attribute names', () => {
	it('are the data attributes the stylesheet selects on', () => {
		expect([SKIN_ATTRIBUTE, MOTION_ATTRIBUTE]).toEqual(['data-skin', 'data-motion']);
	});
});
