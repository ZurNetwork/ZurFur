import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import AcceptingScrollingTabs from '../../checks/testing/AcceptingScrollingTabs.svelte';
import ScrollBoxHost from '../../checks/testing/ScrollBoxHost.svelte';
import type { TabItem } from './types';
import '../styles/index.ts';

type Section =
	'gallery' | 'comms' | 'ocs' | 'collections' | 'posts' | 'reviews' | 'friends' | 'about';

const sections: readonly TabItem<Section>[] = [
	{ id: 'gallery', label: 'Gallery', icon: 'image', count: '24' },
	{ id: 'comms', label: 'Comms', count: '2' },
	{ id: 'ocs', label: 'OCs', count: '6' },
	{ id: 'collections', label: 'Collections' },
	{ id: 'posts', label: 'Posts' },
	{ id: 'reviews', label: 'Reviews', count: '18' },
	{ id: 'friends', label: 'Friends' },
	{ id: 'about', label: 'About' }
];

const stepWords = { back: 'Earlier tabs', forward: 'More tabs' } as const;

/** A box of this width in pixels, in the page, for the tabs to be drawn in. */
function boxOf(width: number, parent: HTMLElement = document.body): HTMLElement {
	const box = document.createElement('div');
	box.setAttribute('data-copy', '');
	box.style.width = `${String(width)}px`;
	parent.append(box);
	return box;
}

interface Spec {
	readonly tabs?: readonly TabItem<Section>[];
	readonly initial?: Section;
	readonly accepts?: boolean;
	readonly words?: { readonly back: string; readonly forward: string };
	readonly label?: string;
}

/** Renders scrolling tabs in `target`; returns the spy that is told of every change asked for. */
function renderTabs(
	target: HTMLElement,
	spec: Spec = {}
): ReturnType<typeof vi.fn<(id: Section) => void>> {
	const onChange = vi.fn<(id: Section) => void>();
	render(AcceptingScrollingTabs<Section>, {
		target,
		props: {
			label: spec.label ?? 'Profile',
			tabs: spec.tabs ?? sections,
			initial: spec.initial ?? 'gallery',
			accepts: spec.accepts ?? true,
			stepWords: spec.words ?? stepWords,
			onChange
		}
	});
	return onChange;
}

/** The tab with this accessible name. */
function tab(name: string): HTMLElement {
	const [found] = page.getByRole('tab', { name }).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`no tab named ${name}`);
	return found;
}

/** The step button with this accessible name. */
function stepButton(name: string): HTMLButtonElement {
	const [found] = page.getByRole('button', { name }).elements();
	if (!(found instanceof HTMLButtonElement)) throw new Error(`no button named ${name}`);
	return found;
}

/** Whether a step button says it stands at its end; it stays focusable there. */
function atEnd(button: HTMLButtonElement): boolean {
	return button.getAttribute('aria-disabled') === 'true';
}

/** Waits for two frames, so a scroll's event has been handled. */
async function frames(): Promise<void> {
	await new Promise((resolve) => requestAnimationFrame(resolve));
	await new Promise((resolve) => requestAnimationFrame(resolve));
}

/**
 * Presses Enter on the step named `name` while it holds focus, until it stands
 * at its end, at most `presses` times; each press must move the list.
 */
async function enterUntilAtEnd(name: string, list: HTMLElement, presses: number): Promise<void> {
	const button = stepButton(name);
	for (
		let left = presses;
		left > 0 && document.activeElement === button && !atEnd(button);
		left -= 1
	) {
		const before = list.scrollLeft;
		await userEvent.keyboard('{Enter}');
		await vi.waitFor(() => {
			expect(list.scrollLeft).not.toBe(before);
		});
		await frames();
	}
}

/** The tab list. */
function tabList(): HTMLElement {
	const [found] = page.getByRole('tablist').elements();
	if (!(found instanceof HTMLElement)) throw new Error('no tab list');
	return found;
}

/** The tabs of the list, in order. */
function tabsOf(list: HTMLElement): readonly HTMLElement[] {
	return [...list.children].filter((child) => child instanceof HTMLElement);
}

/** How far the list is scrolled when `item` sits at its start, read from where the tab is drawn. */
function startOf(list: HTMLElement, item: HTMLElement): number {
	const padding = Number.parseFloat(getComputedStyle(list).scrollPaddingLeft);
	const drawn = item.getBoundingClientRect().left - list.getBoundingClientRect().left;
	return drawn + list.scrollLeft - padding;
}

/** How far the list can be scrolled. */
function furthest(list: HTMLElement): number {
	return list.scrollWidth - list.clientWidth;
}

/** Waits until the list rests within a pixel of `position`. */
async function restsAt(list: HTMLElement, position: number): Promise<void> {
	await vi.waitFor(() => {
		expect(Math.abs(list.scrollLeft - position)).toBeLessThanOrEqual(1);
	});
}

/** Whether `item` lies whole inside the list, to a pixel. */
function isWhollyIn(list: HTMLElement, item: HTMLElement): boolean {
	const around = list.getBoundingClientRect();
	const inside = item.getBoundingClientRect();
	return inside.left >= around.left - 1 && inside.right <= around.right + 1;
}

/** The panels in the document, by their test ids. */
function panelIds(): readonly (string | null)[] {
	return page
		.getByRole('tabpanel')
		.elements()
		.map((found) => found.querySelector('[data-testid]')?.getAttribute('data-testid') ?? null);
}

/** Waits `ms` milliseconds. */
function pause(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

afterEach(() => {
	for (const copy of document.querySelectorAll('[data-copy]')) copy.remove();
	document.scrollingElement?.scrollTo(0, 0);
});

describe('ScrollingTabs', () => {
	it('is the tab list between a back step and a forward step, named by their words', async () => {
		const box = boxOf(260);
		renderTabs(box);
		await expect.element(page.getByRole('tablist', { name: 'Profile' })).toBeInTheDocument();
		const row = tabList().parentElement;
		expect(row?.classList.contains('zf-tabs-row')).toBe(true);
		const kinds = [...(row?.children ?? [])].map((child) =>
			child === tabList() ? 'tabs' : child.getAttribute('aria-label')
		);
		expect(kinds).toEqual(['Earlier tabs', 'tabs', 'More tabs']);
		expect(tabList().querySelectorAll('button')).toHaveLength(sections.length);
		expect(tabList().classList.contains('zf-tabs--step')).toBe(true);
		expect(tabList().querySelector('[aria-label]')).toBeNull();
	});

	it('steps one tab forward at a time and leaves the tab showing alone', async () => {
		const box = boxOf(260);
		const onChange = renderTabs(box);
		const list = tabList();
		const items = tabsOf(list);
		expect(furthest(list)).toBeGreaterThan(0);
		await restsAt(list, 0);
		for (const next of [1, 2, 3]) {
			stepButton('More tabs').click();
			await restsAt(list, startOf(list, items[next] ?? list));
		}
		expect(onChange).not.toHaveBeenCalled();
		await expect
			.element(page.getByRole('tab', { name: 'Gallery' }))
			.toHaveAttribute('aria-selected', 'true');
		expect(panelIds()).toEqual(['panel-gallery']);
	});

	it('steps one tab back at a time and leaves the tab showing alone', async () => {
		const box = boxOf(260);
		const onChange = renderTabs(box, { initial: 'comms' });
		const list = tabList();
		const items = tabsOf(list);
		list.scrollLeft = furthest(list);
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(true);
		});
		const last = items.length - 1;
		const lastStart = startOf(list, items[last] ?? list);
		expect(lastStart).toBeGreaterThanOrEqual(furthest(list));
		// From the far end, the first step back lands on the start of the last tab that begins before it.
		const before = items.findLast((item) => startOf(list, item) < list.scrollLeft - 1) ?? list;
		stepButton('Earlier tabs').click();
		await restsAt(list, startOf(list, before));
		stepButton('Earlier tabs').click();
		const earlier =
			items.findLast((item) => startOf(list, item) < startOf(list, before) - 1) ?? list;
		await restsAt(list, startOf(list, earlier));
		expect(onChange).not.toHaveBeenCalled();
		expect(panelIds()).toEqual(['panel-comms']);
	});

	it('disables each step at its end, and both when every tab fits', async () => {
		const box = boxOf(260);
		renderTabs(box);
		const list = tabList();
		await vi.waitFor(() => {
			expect(atEnd(stepButton('Earlier tabs'))).toBe(true);
		});
		expect(atEnd(stepButton('More tabs'))).toBe(false);
		list.scrollLeft = furthest(list);
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(true);
		});
		expect(atEnd(stepButton('Earlier tabs'))).toBe(false);
		list.scrollLeft = list.scrollLeft / 2;
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(false);
		});
		const wide = boxOf(1400);
		renderTabs(wide, { label: 'Wide', words: { back: 'Back wide', forward: 'Forward wide' } });
		await vi.waitFor(() => {
			expect(atEnd(stepButton('Back wide'))).toBe(true);
		});
		expect(atEnd(stepButton('Forward wide'))).toBe(true);
	});

	it('keeps focus on each step at its end, says so, and moves nothing on another press', async () => {
		const box = boxOf(260);
		renderTabs(box);
		const list = tabList();
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(false);
		});
		stepButton('More tabs').focus();
		await enterUntilAtEnd('More tabs', list, sections.length);
		const forward = stepButton('More tabs');
		expect(document.activeElement).toBe(forward);
		expect(forward.getAttribute('aria-disabled')).toBe('true');
		expect(forward.hasAttribute('disabled')).toBe(false);
		const end = list.scrollLeft;
		expect(Math.abs(end - furthest(list))).toBeLessThanOrEqual(1);
		await userEvent.keyboard('{Enter}');
		await frames();
		expect(list.scrollLeft).toBe(end);
		expect(document.activeElement).toBe(forward);
		stepButton('Earlier tabs').focus();
		await enterUntilAtEnd('Earlier tabs', list, sections.length);
		const back = stepButton('Earlier tabs');
		expect(document.activeElement).toBe(back);
		expect(back.getAttribute('aria-disabled')).toBe('true');
		expect(back.hasAttribute('disabled')).toBe(false);
		expect(list.scrollLeft).toBeLessThanOrEqual(1);
		const start = list.scrollLeft;
		await userEvent.keyboard('{Enter}');
		await frames();
		expect(list.scrollLeft).toBe(start);
		expect(document.activeElement).toBe(back);
	});

	it('keeps up with the room: a box that grows stops needing the steps', async () => {
		const box = boxOf(260);
		renderTabs(box);
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(false);
		});
		box.style.width = '1400px';
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(true);
		});
	});

	it('puts the step buttons in the tab order, tipped, and never in the tab list', async () => {
		const box = boxOf(260);
		renderTabs(box);
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(false);
		});
		tab('Gallery').focus();
		await userEvent.tab();
		expect(document.activeElement).toBe(stepButton('More tabs'));
		await vi.waitFor(
			() => {
				expect(document.querySelector('.zf-tip')?.textContent).toBe('More tabs');
			},
			{ timeout: 3000 }
		);
		await userEvent.keyboard('{Enter}');
		await vi.waitFor(() => {
			expect(tabList().scrollLeft).toBeGreaterThan(1);
		});
		expect(panelIds()).toEqual(['panel-gallery']);
		expect(tabList().contains(stepButton('More tabs'))).toBe(false);
	});

	it('moves and shows the next tab on the right arrow, bringing it whole into view', async () => {
		const box = boxOf(260);
		const onChange = renderTabs(box);
		const list = tabList();
		const items = tabsOf(list);
		const hidden = items.findIndex((item) => !isWhollyIn(list, item));
		expect(hidden).toBeGreaterThan(0);
		const lastWhole = items[hidden - 1] ?? list;
		lastWhole.focus();
		await vi.waitFor(() => {
			expect(document.activeElement).toBe(lastWhole);
		});
		await userEvent.keyboard('{ArrowRight}');
		const next = items[hidden] ?? list;
		expect(document.activeElement).toBe(next);
		expect(onChange).toHaveBeenLastCalledWith(sections[hidden]?.id);
		await vi.waitFor(() => {
			expect(isWhollyIn(list, next)).toBe(true);
		});
		await expect.element(next).toHaveAttribute('aria-selected', 'true');
	});

	it('moves and shows the first and last tab with Home and End, each whole inside the list', async () => {
		const box = boxOf(260);
		const onChange = renderTabs(box);
		const list = tabList();
		const items = tabsOf(list);
		tab('Gallery').focus();
		await userEvent.keyboard('{End}');
		expect(onChange).toHaveBeenLastCalledWith('about');
		await vi.waitFor(() => {
			expect(isWhollyIn(list, items[items.length - 1] ?? list)).toBe(true);
		});
		await userEvent.keyboard('{Home}');
		expect(onChange).toHaveBeenLastCalledWith('gallery');
		await vi.waitFor(() => {
			expect(isWhollyIn(list, items[0] ?? list)).toBe(true);
		});
		await restsAt(list, 0);
	});

	it.each([700, 320])(
		'keeps each tab a key moves to whole inside the list in a box %ipx wide',
		async (width) => {
			const box = boxOf(width);
			renderTabs(box);
			const list = tabList();
			const items = tabsOf(list);
			const first = items[0] ?? list;
			first.focus();
			await vi.waitFor(() => {
				expect(document.activeElement).toBe(first);
			});
			for (const next of items.slice(1)) {
				await userEvent.keyboard('{ArrowRight}');
				expect(document.activeElement).toBe(next);
				expect(isWhollyIn(list, next)).toBe(true);
			}
			await userEvent.keyboard('{Home}');
			expect(document.activeElement).toBe(first);
			expect(isWhollyIn(list, first)).toBe(true);
			await userEvent.keyboard('{End}');
			const last = items[items.length - 1] ?? list;
			expect(document.activeElement).toBe(last);
			expect(isWhollyIn(list, last)).toBe(true);
			await pause(150);
			expect(isWhollyIn(list, last)).toBe(true);
		}
	);

	it('shows one tab panel, as Tabs does', async () => {
		const box = boxOf(260);
		renderTabs(box);
		await page.getByRole('tab', { name: 'Comms' }).click();
		await expect.element(page.getByRole('tabpanel', { name: 'Comms' })).toBeInTheDocument();
		expect(panelIds()).toEqual(['panel-comms']);
		expect(document.querySelectorAll('[role="tabpanel"]')).toHaveLength(1);
	});

	it('scrolls no box but its own tab list as it steps and as a key moves', async () => {
		render(ScrollBoxHost, {});
		const [boxFound] = page.getByTestId('scroll-box').elements();
		const [scopeFound] = page.getByTestId('scroll-box-scope').elements();
		if (!(boxFound instanceof HTMLElement) || !(scopeFound instanceof HTMLElement)) {
			throw new Error('no scroll box');
		}
		// The tabs sit below a gap and above a drop, so the box scrolls, and a scrolled-to-start tab would move it.
		const gap = document.createElement('div');
		gap.style.height = '24px';
		const target = document.createElement('div');
		target.style.width = '260px';
		const drop = document.createElement('div');
		drop.style.height = '300px';
		scopeFound.append(gap, target, drop);
		renderTabs(target);
		expect(boxFound.scrollHeight).toBeGreaterThan(boxFound.clientHeight);
		const list = tabList();
		const items = tabsOf(list);
		await vi.waitFor(() => {
			expect(atEnd(stepButton('More tabs'))).toBe(false);
		});
		stepButton('More tabs').click();
		await vi.waitFor(() => {
			expect(list.scrollLeft).toBeGreaterThan(1);
		});
		stepButton('More tabs').click();
		const hidden = items.findIndex((item) => !isWhollyIn(list, item));
		items[Math.max(hidden - 1, 0)]?.focus();
		await userEvent.keyboard('{ArrowRight}');
		await userEvent.keyboard('{End}');
		await pause(150);
		expect(boxFound.scrollTop).toBe(0);
		expect(document.scrollingElement?.scrollTop).toBe(0);
		expect(list.scrollLeft).toBeGreaterThan(1);
	});

	it('shows markup in its tab words and its step words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		const box = boxOf(260);
		const tabs: readonly TabItem<Section>[] = [
			{ id: 'gallery', label: words, count: words },
			...sections.slice(1)
		];
		renderTabs(box, { tabs, label: words, words: { back: words, forward: `${words} again` } });
		await expect.element(page.getByRole('tablist', { name: words })).toBeInTheDocument();
		expect(tab(words).textContent).toContain(words);
		expect(stepButton(words).getAttribute('aria-label')).toBe(words);
		expect(stepButton(`${words} again`).getAttribute('aria-label')).toBe(`${words} again`);
		stepButton(`${words} again`).focus();
		await vi.waitFor(
			() => {
				expect(document.querySelector('.zf-tip')?.textContent).toBe(`${words} again`);
			},
			{ timeout: 3000 }
		);
		expect(document.querySelector('img')).toBeNull();
	});

	it('wears the tabs’ piece and picks, and adds only the row and the steps', () => {
		const box = boxOf(260);
		renderTabs(box);
		expect([...tabList().classList].sort()).toEqual(['piece', 'zf-tabs', 'zf-tabs--step']);
		expect(tab('OCs').className.split(' ').sort()).toEqual(['pick', 'zf-tab']);
		expect(stepButton('More tabs').className).toContain('zf-iconbtn');
		expect(stepButton('More tabs').className).toContain('zf-iconbtn--sm');
	});

	it('snaps tab by tab with the scroll padding of its piece', () => {
		const box = boxOf(260);
		renderTabs(box);
		const list = tabList();
		const style = getComputedStyle(list);
		expect(style.scrollSnapType).toBe('x mandatory');
		expect(style.scrollPaddingLeft).toBe(style.paddingLeft);
		const first = getComputedStyle(tab('Gallery'));
		expect([first.scrollSnapAlign, first.scrollSnapStop]).toEqual(['start', 'always']);
	});
});
