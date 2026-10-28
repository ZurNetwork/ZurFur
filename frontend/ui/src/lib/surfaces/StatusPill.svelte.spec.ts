import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import StatusPill from './StatusPill.svelte';
import type { IconName } from '../icons/icons.ts';
import type { StatusParts, StatusPart, StatusTone } from './types.ts';
import '../styles/index.ts';

/** The middle dot (U+00B7), built from its code point so this file holds none. */
const MIDDLE_DOT = String.fromCodePoint(0x00b7);

const working: StatusPart = { label: 'Sketching', tone: 'progress', icon: 'circle-dot' };
const waiting: StatusPart = { label: 'Waiting on Mika', tone: 'waiting', icon: 'hourglass' };

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** The parts drawn inside a pill. */
function segmentsIn(root: HTMLElement): readonly Element[] {
	return [...root.querySelectorAll('.zf-pill__seg')];
}

/** The words a part shows, without the pause read out before a second part. */
function shownLabel(segment: Element | undefined): string | undefined {
	return segment?.lastElementChild?.textContent.trim();
}

/** The text of an element as read in order, its runs of white space folded to one. */
function readText(element: Element): string {
	return element.textContent.replace(/\s+/g, ' ').trim();
}

describe('StatusPill', () => {
	it('draws one part with its label, tone and icon', () => {
		render(StatusPill, { parts: [working], testid: 'pill' });
		const [segment, ...rest] = segmentsIn(byTestId('pill'));
		expect(rest).toEqual([]);
		expect(segment?.classList.contains('zf-status--progress')).toBe(true);
		expect(segment?.textContent.trim()).toBe('Sketching');
		expect(segment?.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
	});

	it('draws two parts side by side, each in its own tone', () => {
		render(StatusPill, { parts: [working, waiting], testid: 'pill' });
		const [first, second, ...rest] = segmentsIn(byTestId('pill'));
		expect(rest).toEqual([]);
		expect(first?.classList.contains('zf-status--progress')).toBe(true);
		expect(second?.classList.contains('zf-status--waiting')).toBe(true);
		expect(shownLabel(second)).toBe('Waiting on Mika');
	});

	it('separates its parts by layout: a border, and no separator drawn', () => {
		render(StatusPill, { parts: [working, waiting], testid: 'pill' });
		const pill = byTestId('pill');
		const [, second] = segmentsIn(pill);
		if (second === undefined) throw new Error('the second part is missing');
		expect(getComputedStyle(second).borderLeftWidth).toBe('1px');
		expect(pill.textContent).not.toContain(MIDDLE_DOT);
		const drawn = [...pill.querySelectorAll('span')].filter(
			(span) => !span.classList.contains('zf-sr') && span.children.length === 0
		);
		expect(drawn.map((span) => span.textContent.trim())).toEqual(['Sketching', 'Waiting on Mika']);
	});

	it('reads its two parts with a pause between them, for screen readers only', () => {
		render(StatusPill, { parts: [working, waiting], testid: 'pill' });
		const pill = byTestId('pill');
		expect(readText(pill)).toBe('Sketching, Waiting on Mika');
		const pauses = [...pill.querySelectorAll('.zf-sr')];
		expect(pauses.map((pause) => pause.textContent)).toEqual([', ']);
		expect(pauses[0]?.parentElement).toBe(segmentsIn(pill)[1]);
	});

	it('reads one part with no pause', () => {
		render(StatusPill, { parts: [working], testid: 'pill' });
		const pill = byTestId('pill');
		expect(readText(pill)).toBe('Sketching');
		expect(pill.textContent).not.toContain(',');
		expect(pill.querySelector('.zf-sr')).toBeNull();
	});

	it('draws at most two parts', () => {
		const three = [working, waiting, working] as unknown as StatusParts;
		render(StatusPill, { parts: three, testid: 'pill' });
		expect(segmentsIn(byTestId('pill'))).toHaveLength(2);
	});

	it('draws no pill for no parts', () => {
		const none = [] as unknown as StatusParts;
		const { container } = render(StatusPill, { parts: none });
		expect(container.querySelector('.zf-pill')).toBeNull();
	});

	it.each(['neutral', 'progress', 'waiting', 'done'] as const)('wears the %s tone', (tone) => {
		render(StatusPill, { parts: [{ label: 'Sketching', tone }], testid: 'pill' });
		const [segment] = segmentsIn(byTestId('pill'));
		expect(segment?.classList.contains(`zf-status--${tone}`)).toBe(true);
	});

	it.each(['danger', 'error', 'Done', '', 'toString'])(
		'wears neutral for the tone %j',
		(stranger) => {
			render(StatusPill, {
				parts: [{ label: 'Sketching', tone: stranger as StatusTone }],
				testid: 'pill'
			});
			const [segment] = segmentsIn(byTestId('pill'));
			expect(segment?.classList.contains('zf-status--neutral')).toBe(true);
			const tones = [...(segment?.classList ?? [])].filter((name) =>
				name.startsWith('zf-status--')
			);
			expect(tones).toEqual(['zf-status--neutral']);
		}
	);

	it('has no danger tone by type', () => {
		// @ts-expect-error StatusTone has no danger: if this line stops failing, a danger tone crept in
		const danger: StatusPart = { label: 'Late', tone: 'danger' };
		expect(danger.tone).toBe('danger');
	});

	it('shows each part label and the fact text written as an image tag as text, with no image', () => {
		const markup = '<img src=x onerror=alert(1)>';
		const { container } = render(StatusPill, {
			parts: [
				{ label: markup, tone: 'progress' },
				{ label: markup, tone: 'waiting' }
			],
			fact: { text: markup },
			testid: 'pill'
		});
		expect(container.querySelector('img')).toBeNull();
		const pill = byTestId('pill');
		const [first, second] = segmentsIn(pill);
		expect(first?.textContent.trim()).toBe(markup);
		expect(shownLabel(second)).toBe(markup);
		expect(pill.querySelector('.zf-status__fact')?.textContent.trim()).toBe(markup);
		expect(page.getByText(markup).elements()).toHaveLength(3);
	});

	it('draws no icon on a part without one, or with one the registry lacks', () => {
		render(StatusPill, {
			parts: [
				{ label: 'Sketching', tone: 'progress' },
				{ label: 'Inking', tone: 'done', icon: 'not-an-icon' as IconName }
			],
			testid: 'pill'
		});
		for (const segment of segmentsIn(byTestId('pill'))) {
			expect(segment.querySelector('svg')).toBeNull();
		}
	});

	it('draws a quiet fact beside the pill', () => {
		render(StatusPill, {
			parts: [working],
			fact: { text: 'Due last week', icon: 'clock' },
			testid: 'pill'
		});
		const pill = byTestId('pill');
		const fact = pill.querySelector('.zf-status__fact');
		expect(fact?.textContent.trim()).toBe('Due last week');
		expect(fact?.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
		expect(fact?.closest('.zf-pill')).toBeNull();
	});

	it('draws a fact without an icon', () => {
		render(StatusPill, { parts: [working], fact: { text: 'Due last week' }, testid: 'pill' });
		const fact = byTestId('pill').querySelector('.zf-status__fact');
		expect(fact?.querySelector('svg')).toBeNull();
	});

	it('draws no fact unless given one', () => {
		render(StatusPill, { parts: [working], testid: 'pill' });
		expect(byTestId('pill').querySelector('.zf-status__fact')).toBeNull();
	});

	it('shows no word that was not handed to it', async () => {
		const { container } = render(StatusPill, {
			parts: [working],
			fact: { text: 'Due last week' },
			testid: 'pill'
		});
		expect(container.textContent.replace(/\s+/g, ' ').trim()).toBe('Sketching Due last week');
		await expect.element(page.getByText('Sketching')).toBeInTheDocument();
	});
});
