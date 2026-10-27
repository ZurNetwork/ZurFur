import { describe, expect, it } from 'vitest';
import { isCalendarDay } from './calendar-day.ts';

const days = [
	'2024-02-29',
	'2000-02-29',
	'2026-01-01',
	'2026-04-30',
	'2026-12-31',
	'0001-01-01',
	'9999-12-31'
];

const refused = [
	'2026-02-29',
	'1900-02-29',
	'2026-13-01',
	'2026-00-10',
	'2026-04-31',
	'2026-06-00',
	'2026-1-1',
	'0000-01-01',
	'10000-01-01',
	'+002026-01-01',
	'-0001-01-01',
	' 2026-01-01',
	'2026-01-01 ',
	'2026-01-01\n',
	'2026-01-01T00:00',
	'2026/01/01',
	'20260101',
	'２０２６-０１-０１',
	'٢٠٢٦-٠١-٠١',
	'__proto__',
	''
];

describe('isCalendarDay', () => {
	it.each(days)('accepts %j', (text) => {
		expect(isCalendarDay(text)).toBe(true);
	});

	it.each(refused)('refuses %j', (text) => {
		expect(isCalendarDay(text)).toBe(false);
	});

	it('agrees with the platform on every day of a leap year and a common one', () => {
		const candidates = [2024, 2026].flatMap((year) =>
			Array.from({ length: 12 * 31 }, (_, index) => ({
				year,
				month: Math.floor(index / 31) + 1,
				day: (index % 31) + 1
			}))
		);
		const disagreements = candidates
			.filter(({ year, month, day }) => {
				const text = `${String(year)}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
				const real = new Date(Date.UTC(year, month - 1, day)).getUTCDate() === day;
				return isCalendarDay(text) !== real;
			})
			.map(({ year, month, day }) => [year, month, day]);
		expect(candidates.filter(({ day }) => day === 29)).toHaveLength(24);
		expect(disagreements).toEqual([]);
	});
});
