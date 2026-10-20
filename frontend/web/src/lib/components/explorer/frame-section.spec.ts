import { describe, expect, it } from 'vitest';
import { frameSectionOf } from './frame-section';

describe('frameSectionOf', () => {
	it('puts the account pages in the Accounts section', () => {
		expect(frameSectionOf('/(session)/accounts')).toBe('accounts');
		expect(frameSectionOf('/(session)/accounts/[id]')).toBe('accounts');
	});

	it('puts the report page in a section of its own, so its link is marked current', () => {
		expect(frameSectionOf('/(session)/report-a-bug')).toBe('report');
	});

	it('puts anything else in no section', () => {
		expect(frameSectionOf('/(session)/logout')).toBe('other');
		expect(frameSectionOf(undefined)).toBe('other');
	});
});
