import { describe, expect, it } from 'vitest';
import { nameOrdinal } from '$lib/types/brand';
import { shownNameText } from './shown-name';

describe('shownNameText', () => {
	it('isolates the name and puts the number outside the isolate', () => {
		expect(shownNameText('Test 1', nameOrdinal(2))).toBe('\u2068Test 1\u2069 (2)');
	});

	it('is the isolated name alone without a number', () => {
		expect(shownNameText('Test 1', undefined)).toBe('\u2068Test 1\u2069');
	});

	it('keeps a right-to-left name inside the isolate, the number after it', () => {
		const name = 'طلب رسم';
		const shown = shownNameText(name, nameOrdinal(3));
		expect(shown).toBe(`\u2068${name}\u2069 (3)`);
		expect(shown.indexOf('\u2069')).toBeLessThan(shown.indexOf(' (3)'));
	});
});
