import { describe, expect, it } from 'vitest';
import { nameOrdinal } from '$lib/types/brand';
import { shownNameText } from './shown-name';

describe('shownNameText', () => {
	it('isolates the name and puts the number outside the isolate', () => {
		expect(shownNameText('Test 1', nameOrdinal(2))).toBe('⁨Test 1⁩ (2)');
	});

	it('is the isolated name alone without a number', () => {
		expect(shownNameText('Test 1', undefined)).toBe('⁨Test 1⁩');
	});

	it('keeps a right-to-left name inside the isolate, the number after it', () => {
		const name = 'طلب رسم';
		const shown = shownNameText(name, nameOrdinal(3));
		expect(shown).toBe(`⁨${name}⁩ (3)`);
		expect(shown.indexOf('⁩')).toBeLessThan(shown.indexOf(' (3)'));
	});
});
