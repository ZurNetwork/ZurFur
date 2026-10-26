import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import AcceptingTabs from '../../checks/testing/AcceptingTabs.svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import type { TabItem } from './types';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

type Section = 'gallery' | 'comms' | 'ocs';

const sections: readonly TabItem<Section>[] = [
	{ id: 'gallery', label: 'Gallery', icon: 'image', count: '24' },
	{ id: 'comms', label: 'Comms', count: '2' },
	{ id: 'ocs', label: 'OCs' }
];

interface Spec {
	readonly tabs?: readonly TabItem<Section>[];
	readonly initial?: Section;
	readonly accepts?: boolean;
	readonly testid?: string;
	readonly label?: string;
}

/** Renders the tabs under a parent that takes (or refuses) what it is told; returns the spy. */
function renderTabs(
	spec: Spec = {},
	wrapped?: (call: SeamCall) => void,
	target?: HTMLElement
): ReturnType<typeof vi.fn<(id: Section) => void>> {
	const onChange = vi.fn<(id: Section) => void>();
	const props = {
		label: spec.label ?? 'Profile',
		tabs: spec.tabs ?? sections,
		initial: spec.initial ?? 'gallery',
		accepts: spec.accepts ?? true,
		testid: spec.testid,
		onChange
	};
	const options = target === undefined ? { props } : { target, props };
	if (wrapped === undefined) {
		render(AcceptingTabs<Section>, options);
	} else {
		const recording = { wrapper: RecordingEnvironment, wrapperProps: { onRecord: wrapped } };
		render(AcceptingTabs<Section>, options, recording);
	}
	return onChange;
}

/** The tab with this accessible name. */
function tab(name: string): HTMLElement {
	const [found] = page.getByRole('tab', { name }).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`no tab named ${name}`);
	return found;
}

/** The panels in the document, by their test ids. */
function panelIds(): readonly (string | null)[] {
	return page
		.getByRole('tabpanel')
		.elements()
		.map((found) => found.querySelector('[data-testid]')?.getAttribute('data-testid') ?? null);
}

afterEach(() => {
	for (const copy of document.querySelectorAll('[data-copy]')) copy.remove();
});

describe('Tabs', () => {
	it('is a tab list named by its label, with a tab for each entry', async () => {
		renderTabs();
		await expect.element(page.getByRole('tablist', { name: 'Profile' })).toBeInTheDocument();
		expect(
			page
				.getByRole('tab')
				.elements()
				.map((found) => found.textContent.replace(/\s+/g, ' ').trim())
		).toEqual(['Gallery 24', 'Comms 2', 'OCs']);
		await expect
			.element(page.getByRole('tab', { name: 'Gallery' }))
			.toHaveAttribute('aria-selected', 'true');
		await expect
			.element(page.getByRole('tab', { name: 'Comms' }))
			.toHaveAttribute('aria-selected', 'false');
	});

	it('shows a muted count and a small icon, and leaves them off where none is given', () => {
		renderTabs();
		const gallery = tab('Gallery');
		expect(gallery.querySelector('.zf-tab__count')?.textContent).toBe('24');
		expect(gallery.querySelector('svg.zf-icon')).not.toBeNull();
		expect(tab('OCs').querySelector('.zf-tab__count')).toBeNull();
		expect(tab('OCs').querySelector('svg')).toBeNull();
	});

	it('shows one tab panel, labelled by its tab, and no other panel is in the document', async () => {
		const { container } = renderTabsAndContainer();
		const panel = page.getByRole('tabpanel', { name: 'Gallery' });
		await expect.element(panel).toHaveTextContent('Gallery would show here.');
		await expect.element(panel).toHaveAttribute('aria-labelledby', tab('Gallery').id);
		await expect.element(tab('Gallery')).toHaveAttribute('aria-controls', panel.element().id);
		expect(container.querySelectorAll('[role="tabpanel"]')).toHaveLength(1);
		expect(panelIds()).toEqual(['panel-gallery']);
	});

	it('hands the parent the id a click asks for, as the typed id, and shows its panel', async () => {
		const onChange = renderTabs();
		await page.getByRole('tab', { name: 'Comms' }).click();
		expect(onChange.mock.calls).toEqual([['comms']]);
		await expect.element(page.getByRole('tabpanel', { name: 'Comms' })).toBeInTheDocument();
		expect(panelIds()).toEqual(['panel-comms']);
	});

	it('moves and shows the next tab with the right arrow', async () => {
		const onChange = renderTabs();
		tab('Gallery').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onChange.mock.calls).toEqual([['comms']]);
		expect(document.activeElement).toBe(tab('Comms'));
		await expect
			.element(page.getByRole('tab', { name: 'Comms' }))
			.toHaveAttribute('aria-selected', 'true');
		expect(panelIds()).toEqual(['panel-comms']);
	});

	it('moves and shows the previous tab with the left arrow, wrapping round the ends', async () => {
		const onChange = renderTabs();
		tab('Gallery').focus();
		await userEvent.keyboard('{ArrowLeft}');
		expect(onChange.mock.calls).toEqual([['ocs']]);
		expect(document.activeElement).toBe(tab('OCs'));
		expect(panelIds()).toEqual(['panel-ocs']);
	});

	it('moves and shows the last and first tab with End and Home', async () => {
		const onChange = renderTabs({ initial: 'comms' });
		tab('Comms').focus();
		await userEvent.keyboard('{End}');
		await userEvent.keyboard('{Home}');
		expect(onChange.mock.calls).toEqual([['ocs'], ['gallery']]);
		expect(panelIds()).toEqual(['panel-gallery']);
	});

	it('takes focus on the showing tab alone when tabbed to', () => {
		renderTabs({ initial: 'comms' });
		const tabStops = page
			.getByRole('tab')
			.elements()
			.filter((found) => found.getAttribute('tabindex') === '0');
		expect(tabStops).toEqual([tab('Comms')]);
	});

	it('stays on the prop’s value when the parent refuses an arrow, though it still asks', async () => {
		const onChange = renderTabs({ accepts: false });
		tab('Gallery').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onChange.mock.calls).toEqual([['comms']]);
		await expect
			.element(page.getByRole('tab', { name: 'Gallery' }))
			.toHaveAttribute('aria-selected', 'true');
		await expect
			.element(page.getByRole('tab', { name: 'Comms' }))
			.toHaveAttribute('aria-selected', 'false');
		expect(panelIds()).toEqual(['panel-gallery']);
	});

	it('stays on the prop’s value when the parent refuses a click', async () => {
		const onChange = renderTabs({ accepts: false });
		await page.getByRole('tab', { name: 'OCs' }).click();
		// bits-ui asks once on the press's focus and again on the click when nothing took the first.
		expect(new Set(onChange.mock.calls.flat())).toEqual(new Set(['ocs']));
		await expect
			.element(page.getByRole('tab', { name: 'Gallery' }))
			.toHaveAttribute('aria-selected', 'true');
		expect(panelIds()).toEqual(['panel-gallery']);
	});

	it('asks nothing for a tab whose id is empty', async () => {
		const tabs: readonly TabItem<Section>[] = [
			{ id: 'gallery', label: 'Gallery' },
			// An id the DOM could hand back as "": the lookup must refuse it, never send it on.
			{ id: '' as Section, label: 'Unnamed' }
		];
		const onChange = renderTabs({ tabs });
		await page.getByRole('tab', { name: 'Unnamed' }).click();
		tab('Gallery').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onChange).not.toHaveBeenCalled();
	});

	it('draws no panel when the value is none of its tabs', () => {
		const { container } = renderTabsAndContainer({ initial: 'ocs', tabs: sections.slice(0, 2) });
		expect(container.querySelectorAll('[role="tabpanel"]')).toHaveLength(0);
	});

	it('puts its test id on the tab list', async () => {
		renderTabs({ testid: 'profile-tabs' });
		await expect.element(page.getByTestId('profile-tabs')).toHaveAttribute('role', 'tablist');
	});

	it('shows markup in its words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		const tabs: readonly TabItem<Section>[] = [
			{ id: 'gallery', label: words, count: words },
			{ id: 'comms', label: 'Comms' }
		];
		const screen = render(AcceptingTabs<Section>, {
			props: { label: words, tabs, initial: 'gallery', accepts: true, onChange: () => undefined }
		});
		await expect.element(page.getByRole('tablist', { name: words })).toBeInTheDocument();
		expect(tab(words).textContent).toContain(words);
		await expect.element(page.getByRole('tabpanel', { name: words })).toBeInTheDocument();
		expect(screen.container.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onChange = renderTabs();
		await page.getByRole('tab', { name: 'OCs' }).click();
		expect(onChange).toHaveBeenCalledWith('ocs');
	});

	it('squishes each tab and plays the toggle cue on a change, and nothing else', async () => {
		const calls: SeamCall[] = [];
		renderTabs({}, (call) => calls.push(call));
		const squishes = page
			.getByRole('tab')
			.elements()
			.map((element) => ({ kind: 'move', move: 'squish', args: [], element }));
		expect(calls).toEqual(squishes);
		tab('Gallery').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(calls.slice(squishes.length)).toEqual([{ kind: 'sound', cue: 'toggle' }]);
	});

	it('wears one piece of picks', () => {
		renderTabs();
		const list = page.getByRole('tablist').element();
		expect([...list.classList].sort()).toEqual(['piece', 'zf-tabs']);
		expect(tab('OCs').className.split(' ').sort()).toEqual(['pick', 'zf-tab']);
	});

	it('scrolls sideways when its tabs outgrow the screen', () => {
		const copy = document.createElement('div');
		copy.setAttribute('data-copy', '');
		copy.style.width = '120px';
		document.body.append(copy);
		renderTabs({}, undefined, copy);
		const list = page.getByRole('tablist').element();
		expect(getComputedStyle(list).overflowX).toBe('auto');
		expect(list.scrollWidth).toBeGreaterThan(list.clientWidth);
	});

	it.each([
		['light', 'rgb(134, 88, 18)', 'rgb(245, 225, 179)'],
		['dark', 'rgb(231, 182, 86)', 'rgb(61, 53, 40)']
	])('shows the showing tab in the %s skin’s accent, bold', (skin, text, tint) => {
		const copy = document.createElement('div');
		copy.setAttribute('data-skin', skin);
		copy.setAttribute('data-copy', '');
		document.body.append(copy);
		renderTabs({ initial: 'comms' }, undefined, copy);
		const showing = getComputedStyle(tab('Comms'));
		expect([showing.color, showing.backgroundColor, showing.fontWeight]).toEqual([
			text,
			tint,
			'700'
		]);
		const resting = getComputedStyle(tab('OCs'));
		expect(resting.color).not.toBe(text);
		expect(resting.fontWeight).toBe('400');
		expect(resting.backgroundColor).toBe('rgba(0, 0, 0, 0)');
	});

	it('keeps a count muted like its resting tab', () => {
		renderTabs();
		const count = getComputedStyle(tab('Comms').querySelector('.zf-tab__count') ?? document.body);
		const text = getComputedStyle(tab('Comms'));
		expect(count.color).toBe(text.color);
	});
});

/** Like `renderTabs`, handing back the render's container. */
function renderTabsAndContainer(spec: Spec = {}): { readonly container: HTMLElement } {
	const props = {
		label: 'Profile',
		tabs: spec.tabs ?? sections,
		initial: spec.initial ?? 'gallery',
		accepts: true,
		onChange: () => undefined
	};
	return render(AcceptingTabs<Section>, { props });
}
