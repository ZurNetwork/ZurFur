/** A calendar day written as text, and the one way to mint it. */

declare const calendarDayBrand: unique symbol;

/** A real day in the Gregorian calendar, written `YYYY-MM-DD`, from 0001-01-01 to 9999-12-31. */
export type CalendarDay = string & { readonly [calendarDayBrand]: true };

/** Four ASCII digits, a hyphen, two, a hyphen, two, and nothing else. */
const DAY_SHAPE = /^(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})$/;

/** Whether `year` has a 29th of February: every fourth year, but not a century unless it divides by 400. */
function isLeapYear(year: number): boolean {
	return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
}

/** How many days `month` (1 to 12) has in `year`. */
function daysInMonth(year: number, month: number): number {
	if (month === 2) return isLeapYear(year) ? 29 : 28;
	return [4, 6, 9, 11].includes(month) ? 30 : 31;
}

/** Whether `text` is a calendar day: `YYYY-MM-DD` in ASCII digits, naming a day that exists, in the years 0001 to 9999. */
export function isCalendarDay(text: string): text is CalendarDay {
	const parts = DAY_SHAPE.exec(text)?.groups;
	if (parts === undefined) return false;
	const year = Number(parts.year);
	const month = Number(parts.month);
	const day = Number(parts.day);
	if (year < 1 || month < 1 || month > 12) return false;
	return day >= 1 && day <= daysInMonth(year, month);
}
