import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync, tick, type ComponentProps } from 'svelte';
import AcceptingDateField from '../../checks/testing/AcceptingDateField.svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import DateField from './DateField.svelte';
import { isCalendarDay, type CalendarDay } from './calendar-day.ts';
import { isLocaleTag, type LocaleTag } from './locale-tag.ts';
import type { DateFieldWords } from './types.ts';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

function day(text: string): CalendarDay {
	if (!isCalendarDay(text)) throw new Error(`not a calendar day: ${text}`);
	return text;
}

function localeTag(text: string): LocaleTag {
	if (!isLocaleTag(text)) throw new Error(`not a locale tag: ${text}`);
	return text;
}

/** German words, so a word of bits-ui's English shows up by not being one of them. */
const words: DateFieldWords = { day: 'Tag', month: 'Monat', year: 'Jahr', empty: 'leer' };
const label = 'Fälligkeit';
const german = localeTag('de-DE');

/** The field's three segments, by the names they are given. */
function segments(): {
	readonly day: HTMLElement;
	readonly month: HTMLElement;
	readonly year: HTMLElement;
} {
	const find = (word: string): HTMLElement => {
		const found = page.getByRole('spinbutton', { name: `${word} ${label}` }).element();
		if (!(found instanceof HTMLElement)) throw new Error(`no ${word} segment`);
		return found;
	};
	return { day: find(words.day), month: find(words.month), year: find(words.year) };
}

/** What the three segments show, day first. */
function shown(): readonly string[] {
	const { day: dayPart, month, year } = segments();
	return [dayPart, month, year].map((element) => element.textContent);
}

/** Types `keys` into the day segment, one key at a time, as a person would. */
async function typeFromDay(keys: string): Promise<void> {
	segments().day.focus();
	await userEvent.keyboard(keys);
	flushSync();
	await tick();
}

/** Moves focus out of the field, so the edit settles. */
async function leave(): Promise<void> {
	const focused = document.activeElement;
	if (focused instanceof HTMLElement) focused.blur();
	flushSync();
	await tick();
	await tick();
}

function inSkin(skin: 'dark' | 'light'): HTMLElement {
	const scope = document.createElement('div');
	scope.setAttribute('data-skin', skin);
	document.body.append(scope);
	return scope;
}

function tokenColour(scope: HTMLElement, token: string): string {
	const probe = document.createElement('span');
	probe.style.color = `var(${token})`;
	scope.append(probe);
	const colour = getComputedStyle(probe).color;
	probe.remove();
	return colour;
}

/** The props every spec starts from: an empty German field whose parent refuses. */
function props(
	overrides: Partial<ComponentProps<typeof DateField>> = {}
): ComponentProps<typeof DateField> {
	return {
		label,
		locale: german,
		words,
		value: undefined,
		onChange: () => undefined,
		...overrides
	};
}

describe('DateField', () => {
	it('is a group named by its label, holding three spinbuttons named by the words', async () => {
		render(DateField, props({ value: day('2026-03-04') }));
		await expect.element(page.getByRole('group', { name: label })).toBeInTheDocument();
		const { day: dayPart, month, year } = segments();
		expect([dayPart, month, year].map((element) => element.getAttribute('aria-label'))).toEqual([
			'Tag',
			'Monat',
			'Jahr'
		]);
		expect(shown()).toEqual(['04', '03', '2026']);
	});

	it('puts the segments in the order the locale writes a date', () => {
		render(DateField, props({ value: day('2026-03-04'), locale: localeTag('en-US') }));
		const spinbuttons = [...document.querySelectorAll('[role="spinbutton"]')];
		expect(spinbuttons.map((element) => element.getAttribute('aria-label'))).toEqual([
			'Monat',
			'Tag',
			'Jahr'
		]);
	});

	it.each([
		['de-DE', ['Tag', 'Monat', 'Jahr']],
		['en-US', ['Monat', 'Tag', 'Jahr']]
	])(
		'shows each empty segment its own word in the order %s writes a date, and says leer',
		(tag, order) => {
			render(DateField, props({ locale: localeTag(tag) }));
			const spinbuttons = [...document.querySelectorAll('[role="spinbutton"]')];
			expect(spinbuttons.map((element) => element.textContent)).toEqual(order);
			expect(spinbuttons.map((element) => element.getAttribute('aria-valuetext'))).toEqual([
				'leer',
				'leer',
				'leer'
			]);
			expect(spinbuttons.map((element) => element.getAttribute('aria-label'))).toEqual(order);
		}
	);

	it('shows digits in a typed day, and the words in the month and year still empty', async () => {
		render(DateField, props());
		await typeFromDay('04');
		expect(shown()).toEqual(['04', 'Monat', 'Jahr']);
		const { day: dayPart, month, year } = segments();
		expect([dayPart, month, year].map((element) => element.getAttribute('aria-valuetext'))).toEqual(
			[null, 'leer', 'leer']
		);
		expect(
			[dayPart, month, year].map((element) => element.classList.contains('zf-date__segment--empty'))
		).toEqual([false, true, true]);
	});

	it('shows no word but the props, in the field or anything it points at', async () => {
		const { container } = render(DateField, props({ hint: 'Bis wann?' }));
		await typeFromDay('04');
		const field = page.getByRole('group', { name: label }).element();
		const pointedAt = ['aria-describedby', 'aria-labelledby', 'aria-errormessage']
			.flatMap((attribute) =>
				[field, ...field.querySelectorAll('*')].map((element) => element.getAttribute(attribute))
			)
			.flatMap((ids) => (ids ?? '').split(/\s+/))
			.filter((id) => id !== '')
			.map((id) => document.getElementById(id)?.textContent ?? '');
		const spoken = [...container.querySelectorAll('*')].flatMap((element) =>
			['aria-label', 'aria-valuetext', 'aria-description', 'title', 'placeholder'].map(
				(attribute) => element.getAttribute(attribute) ?? ''
			)
		);
		const wordsIn = (text: string): readonly string[] =>
			text.split(/[\s\p{P}\p{Decimal_Number}]+/u).filter((word) => /\p{L}/u.test(word));
		const allowed = new Set(
			[label, 'Bis wann?', words.day, words.month, words.year, words.empty].flatMap(wordsIn)
		);
		const texts = [container.textContent, ...pointedAt, ...spoken];
		expect(texts.flatMap(wordsIn).filter((word) => !allowed.has(word))).toEqual([]);
	});

	it('leaves bits-ui’s own English outside the field: the announcer’s Empty and a hidden Selected Date', async () => {
		const { container } = render(DateField, props({ value: day('2026-03-04') }));
		await typeFromDay('{Backspace}');
		// bits-ui 2.19.5's segments keep the announcer made before mount, which has no document,
		// so its live region stays empty; were that fixed, "Empty" is the one word it may add.
		const logs = [...document.querySelectorAll('[role="log"]')];
		expect(logs).toHaveLength(2);
		expect(['', 'Empty']).toContain(logs.map((log) => log.textContent).join(''));
		expect(logs.filter((log) => container.contains(log))).toEqual([]);
		const described = [...document.body.children].filter((element) =>
			element.textContent.startsWith('Selected Date')
		);
		expect(described).toHaveLength(1);
		const [description] = described;
		expect(description === undefined ? '' : getComputedStyle(description).display).toBe('none');
		const pointers = [...container.querySelectorAll('[aria-describedby]')].map(
			(element) => element.getAttribute('aria-describedby') ?? ''
		);
		expect(pointers.filter((ids) => ids.includes(description?.id ?? '-'))).toEqual([]);
	});

	it('hands the parent the day typed, as a calendar day', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(AcceptingDateField, {
			label,
			locale: german,
			words,
			initial: undefined,
			onChange
		});
		await typeFromDay('04032026');
		expect(onChange).toHaveBeenLastCalledWith('2026-03-04');
		expect(shown()).toEqual(['04', '03', '2026']);
	});

	it('shows the parent’s day once the edit is over, when the parent refuses', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(DateField, props({ value: day('2026-03-04'), onChange }));
		await typeFromDay('15');
		expect(onChange).toHaveBeenLastCalledWith('2026-03-15');
		expect(shown()).toEqual(['15', '03', '2026']);
		await leave();
		expect(shown()).toEqual(['04', '03', '2026']);
	});

	it('stays empty once the edit is over, when the parent refuses a first day', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(DateField, props({ onChange }));
		await typeFromDay('04032026');
		expect(onChange).toHaveBeenLastCalledWith('2026-03-04');
		await leave();
		expect(shown()).toEqual(['Tag', 'Monat', 'Jahr']);
	});

	it('reports undefined once a segment is cleared', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(AcceptingDateField, {
			label,
			locale: german,
			words,
			initial: day('2026-03-04'),
			onChange
		});
		await typeFromDay('{Backspace}');
		expect(onChange.mock.calls).toEqual([[undefined]]);
		expect(shown()).toEqual(['Tag', '03', '2026']);
		await leave();
		expect(shown()).toEqual(['Tag', '03', '2026']);
	});

	it('hands on no day before min, and shows the parent’s day once the edit is over', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(AcceptingDateField, {
			label,
			locale: german,
			words,
			initial: undefined,
			min: day('2026-01-01'),
			onChange
		});
		await typeFromDay('31122025');
		expect(onChange).not.toHaveBeenCalled();
		expect(shown()).toEqual(['31', '12', '2025']);
		await leave();
		expect(shown()).toEqual(['Tag', 'Monat', 'Jahr']);
	});

	it('hands on no day after max', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(DateField, props({ value: day('2026-03-04'), max: day('2026-12-31'), onChange }));
		segments().year.focus();
		await userEvent.keyboard('2027');
		expect(onChange.mock.calls.map(([next]) => next)).toEqual([
			'0002-03-04',
			'0020-03-04',
			'0202-03-04'
		]);
	});

	it('skips the years a typed year passes through below min', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(AcceptingDateField, {
			label,
			locale: german,
			words,
			initial: undefined,
			min: day('2000-01-01'),
			onChange
		});
		await typeFromDay('04032026');
		expect(onChange.mock.calls).toEqual([['2026-03-04']]);
		expect(shown()).toEqual(['04', '03', '2026']);
	});

	it('empties when the parent clears its day while nobody is editing', async () => {
		let setDay: (next: CalendarDay | undefined) => void = () => undefined;
		render(AcceptingDateField, {
			label,
			locale: german,
			words,
			initial: undefined,
			onChange: () => undefined,
			onReady: (set: (next: CalendarDay | undefined) => void) => {
				setDay = set;
			}
		});
		await typeFromDay('04032026');
		await leave();
		expect(shown()).toEqual(['04', '03', '2026']);
		setDay(undefined);
		flushSync();
		await tick();
		expect(shown()).toEqual(['Tag', 'Monat', 'Jahr']);
		setDay(day('2025-12-24'));
		flushSync();
		await tick();
		expect(shown()).toEqual(['24', '12', '2025']);
	});

	it('steps an empty segment from startFrom', async () => {
		render(DateField, props({ startFrom: day('2030-07-09') }));
		segments().day.focus();
		await userEvent.keyboard('{ArrowUp}');
		expect(shown()).toEqual(['09', 'Monat', 'Jahr']);
	});

	it('moves between segments with the arrow keys', async () => {
		render(DateField, props({ value: day('2026-03-04') }));
		segments().day.focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(document.activeElement).toBe(segments().month);
		await userEvent.keyboard('{ArrowLeft}');
		expect(document.activeElement).toBe(segments().day);
	});

	it('is described by its hint and its error, and invalid with an error', async () => {
		render(DateField, props({ value: day('2026-03-04'), hint: 'Bis wann?', error: 'Zu spät.' }));
		const field = page.getByRole('group', { name: label });
		await expect.element(field).toHaveAccessibleDescription('Bis wann? Zu spät.');
		expect(segments().day.getAttribute('aria-invalid')).toBe('true');
	});

	it('is required on every segment when required', () => {
		render(DateField, props({ required: true }));
		const { day: dayPart, month, year } = segments();
		expect([dayPart, month, year].map((element) => element.getAttribute('aria-required'))).toEqual([
			'true',
			'true',
			'true'
		]);
	});

	it('asks nothing while disabled', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(DateField, props({ value: day('2026-03-04'), disabled: true, onChange }));
		const { day: dayPart } = segments();
		expect(dayPart.getAttribute('aria-disabled')).toBe('true');
		dayPart.focus();
		await userEvent.keyboard('15');
		expect(onChange).not.toHaveBeenCalled();
		expect(shown()).toEqual(['04', '03', '2026']);
	});

	it('can be taken off the page in the middle of an edit', async () => {
		const screen = render(DateField, props({ value: day('2026-03-04') }));
		await typeFromDay('15');
		expect(document.activeElement).toBe(segments().month);
		screen.unmount();
		await tick();
		await tick();
		expect(document.querySelector('[role="spinbutton"]')).toBeNull();
	});

	it('posts the day in a hidden input only when it has a name', () => {
		const { container } = render(DateField, props({ value: day('2026-03-04') }));
		expect(container.querySelectorAll('input')).toHaveLength(0);
		const named = render(DateField, props({ value: day('2026-03-04'), name: 'due' }));
		const hidden = named.container.querySelector('input');
		expect(hidden?.type).toBe('hidden');
		expect(hidden?.name).toBe('due');
		expect(hidden?.value).toBe('2026-03-04');
	});

	it('posts an empty value while the field holds no day', () => {
		const { container } = render(DateField, props({ name: 'due' }));
		expect(container.querySelector('input')?.value).toBe('');
	});

	it('puts its test id on the group', async () => {
		render(DateField, props({ testid: 'due-field' }));
		await expect.element(page.getByTestId('due-field')).toHaveAttribute('role', 'group');
	});

	it('shows markup in its words as text', async () => {
		const markup = '<img src=x onerror=alert(1)>';
		const { container } = render(DateField, {
			label: markup,
			locale: german,
			words: { day: markup, month: markup, year: markup, empty: markup },
			value: undefined,
			onChange: () => undefined,
			hint: markup,
			error: markup
		});
		await expect.element(page.getByRole('group', { name: markup })).toBeInTheDocument();
		expect(container.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onChange = vi.fn<(next: CalendarDay | undefined) => void>();
		render(DateField, props({ value: day('2026-03-04'), onChange }));
		await typeFromDay('15');
		expect(onChange).toHaveBeenLastCalledWith('2026-03-15');
	});

	it('calls nothing in the seam', async () => {
		const calls: SeamCall[] = [];
		render(
			AcceptingDateField,
			{ label, locale: german, words, initial: undefined, onChange: () => undefined },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		await typeFromDay('04032026{Backspace}');
		await leave();
		expect(calls).toEqual([]);
	});

	it('wears the bundle classes, a slip with the one focus ring on the field', () => {
		const { container } = render(DateField, props({ value: day('2026-03-04') }));
		const frame = container.querySelector('.zf-input');
		expect(frame?.classList.contains('zf-date')).toBe(true);
		expect(frame?.classList.contains('slip')).toBe(true);
		segments().day.focus();
		const ring = getComputedStyle(frame ?? document.body);
		expect(ring.outlineStyle).toBe('solid');
		expect(ring.outlineWidth).toBe('2px');
		expect(getComputedStyle(segments().day).outlineColor).toBe('rgba(0, 0, 0, 0)');
	});

	it.each([['dark'], ['light']] as const)('takes its colours from the %s skin', (skin) => {
		const scope = inSkin(skin);
		const { container } = render(DateField, {
			target: scope,
			props: props({ value: day('2026-03-04'), error: 'Zu spät.' })
		});
		const frame = container.querySelector('.zf-date') ?? scope;
		expect(getComputedStyle(frame).backgroundColor).toBe(tokenColour(scope, '--color-object'));
		expect(getComputedStyle(frame).borderColor).toBe(tokenColour(scope, '--color-danger-text'));
		segments().day.focus();
		for (const fade of segments().day.getAnimations()) fade.finish();
		expect(getComputedStyle(segments().day).backgroundColor).toBe(
			tokenColour(scope, '--color-selected')
		);
		expect(getComputedStyle(segments().month).color).toBe(tokenColour(scope, '--color-text'));
		scope.remove();
	});
});
