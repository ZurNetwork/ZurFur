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
		expect(second?.textContent.trim()).toBe('Waiting on Mika');
	});

	it('separates its parts by layout: a border, and no separator text', () => {
		render(StatusPill, { parts: [working, waiting], testid: 'pill' });
		const pill = byTestId('pill');
		const [, second] = segmentsIn(pill);
		if (second === undefined) throw new Error('the second part is missing');
		expect(getComputedStyle(second).borderLeftWidth).toBe('1px');
		expect(pill.textContent.replace(/\s+/g, ' ').trim()).toBe('Sketching Waiting on Mika');
		expect(pill.textContent).not.toContain(MIDDLE_DOT);
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
