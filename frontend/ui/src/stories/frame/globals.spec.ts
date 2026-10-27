import { describe, expect, it } from 'vitest';
import { parseMotion, parseSkinChoice } from './globals.ts';

/** Text and values a page's address or a stale global could hand over. */
const strangers: readonly unknown[] = [
	'Dark',
	'LIGHT',
	'',
	'%3Cx',
	'<x>',
	' dark',
	'dark ',
	'os ',
	'still ',
	'Still',
	'lively',
	undefined,
	7,
	true,
	{},
	['dark'],
	['still']
];

describe('parseSkinChoice', () => {
	it.each([
		['os', 'os'],
		['dark', 'dark'],
		['light', 'light']
	])('reads %j as %j', (raw, expected) => {
		expect(parseSkinChoice(raw)).toBe(expected);
	});

	it.each(strangers)('follows the OS for %j', (raw) => {
		expect(parseSkinChoice(raw)).toBe('os');
	});
});

describe('parseMotion', () => {
	it.each([
		['lively', 'lively'],
		['still', 'still']
	])('reads %j as %j', (raw, expected) => {
		expect(parseMotion(raw)).toBe(expected);
	});

	it.each(strangers)('stays lively for %j', (raw) => {
		expect(parseMotion(raw)).toBe('lively');
	});
});
