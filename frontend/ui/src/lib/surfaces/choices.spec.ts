import { describe, expect, it } from 'vitest';
import {
	SKELETON_LINES_CAP,
	badgeToneOf,
	cardElementOf,
	iconOf,
	skeletonLinesOf,
	statusToneOf,
	visibilityIconOf,
	visibilityOf
} from './choices.ts';

/** Values no prop type allows, as a page could still hand them over at run time. */
const strangers: readonly unknown[] = [
	'img',
	'a',
	'button',
	'DIV',
	'Section',
	' div',
	'',
	undefined,
	'toString',
	'__proto__',
	0,
	1,
	true,
	{},
	[],
	Symbol('x')
];

describe('cardElementOf', () => {
	it.each(['div', 'section', 'article', 'li'] as const)('keeps %s', (element) => {
		expect(cardElementOf(element)).toBe(element);
	});

	it.each(strangers)('turns %o into a div', (stranger) => {
		expect(cardElementOf(stranger)).toBe('div');
	});

	it('turns null into a div', () => {
		const nothing: unknown = JSON.parse('null');
		expect(cardElementOf(nothing)).toBe('div');
	});
});

describe('badgeToneOf', () => {
	it.each(['neutral', 'accent', 'iris', 'crown', 'rose', 'taupe'] as const)('keeps %s', (tone) => {
		expect(badgeToneOf(tone)).toBe(tone);
	});

	it.each([...strangers, 'danger', 'Accent', 'progress'])('turns %o into neutral', (stranger) => {
		expect(badgeToneOf(stranger)).toBe('neutral');
	});
});

describe('statusToneOf', () => {
	it.each(['neutral', 'progress', 'waiting', 'done'] as const)('keeps %s', (tone) => {
		expect(statusToneOf(tone)).toBe(tone);
	});

	it.each([...strangers, 'danger', 'error', 'accent', 'Done'])(
		'turns %o into neutral',
		(stranger) => {
			expect(statusToneOf(stranger)).toBe('neutral');
		}
	);
});

describe('visibilityOf', () => {
	it.each(['private', 'listed', 'public'] as const)('keeps %s', (level) => {
		expect(visibilityOf(level)).toBe(level);
	});

	it.each([...strangers, 'Private', 'hidden'])('names nothing for %o', (stranger) => {
		expect(visibilityOf(stranger)).toBeUndefined();
	});

	it('gives each level its own icon', () => {
		expect(visibilityIconOf('private')).toBe('eye-off');
		expect(visibilityIconOf('listed')).toBe('list');
		expect(visibilityIconOf('public')).toBe('globe');
	});
});

describe('iconOf', () => {
	it('keeps a registered icon', () => {
		expect(iconOf('clock')).toBe('clock');
	});

	it.each([...strangers, 'not-an-icon', 'Clock'])('names nothing for %o', (stranger) => {
		expect(iconOf(stranger)).toBeUndefined();
	});
});

describe('skeletonLinesOf', () => {
	it.each([1, 2, 3, SKELETON_LINES_CAP])('keeps %d', (count) => {
		expect(skeletonLinesOf(count)).toBe(count);
	});

	it.each([
		0,
		-1,
		1.5,
		SKELETON_LINES_CAP + 1,
		1000,
		Number.NaN,
		Number.POSITIVE_INFINITY,
		'3',
		'',
		undefined,
		true,
		{},
		[]
	])('draws one bar for %o', (stranger) => {
		expect(skeletonLinesOf(stranger)).toBe(1);
	});
});
