import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { cdp, page } from 'vitest/browser';
import { beforeEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { denHref, nameOrdinal } from '$lib/types/brand';
import type { Trail } from '$lib/api/trail';
import PathBar from './PathBar.svelte';

/** `~ / accounts / Alice's Studio`, linked down to the parent. */
const deepTrail: Trail = [
	{ step: 'root', href: denHref('/den') },
	{ step: 'named', label: 'accounts', ordinal: undefined, href: '/accounts' },
	{ step: 'named', label: "Alice's Studio", ordinal: undefined, href: undefined }
];

/** The text of the step marked `aria-current="page"`, if any. */
function currentStep(): string | undefined {
	const bar = page.getByTestId('path-bar').element();
	return bar.querySelector('[aria-current="page"]')?.textContent ?? undefined;
}

/** A parent whose name is far wider than a phone. */
const longParentTrail: Trail = [
	{ step: 'root', href: denHref('/den') },
	{
		step: 'named',
		label: 'Supreme Arts and Crafts Collective of the Northern Reaches',
		ordinal: undefined,
		href: '/accounts'
	},
	{ step: 'named', label: 'slots', ordinal: undefined, href: undefined }
];

/** The viewport box of an element's focus ring: its border box grown by the outline. */
function ringBox(element: Element): DOMRect {
	const style = getComputedStyle(element);
	const reach = parseFloat(style.outlineWidth) + parseFloat(style.outlineOffset);
	const box = element.getBoundingClientRect();
	return new DOMRect(box.x - reach, box.y - reach, box.width + 2 * reach, box.height + 2 * reach);
}

/** The text nodes Chromium's accessibility tree exposes to a screen reader, in this test's frame. */
async function spokenTexts(): Promise<string[]> {
	const session = cdp();
	await session.send('Accessibility.enable');
	const { frameTree } = await session.send('Page.getFrameTree');
	const ownFrame = frameTree.childFrames?.find((child) => child.frame.url === location.href);
	if (ownFrame === undefined) throw new Error("the test's frame is not in the page's frame tree");
	const tree = await session.send('Accessibility.getFullAXTree', { frameId: ownFrame.frame.id });
	return tree.nodes
		.filter((node) => !node.ignored && node.role?.value === 'StaticText')
		.map((node) => String(node.name?.value ?? ''));
}

describe('PathBar', () => {
	beforeEach(async () => {
		await page.viewport(1280, 800);
	});

	it('lists every step, the last marked as the current page', async () => {
		render(PathBar, { trail: deepTrail });

		const path = page.getByTestId('path-bar');
		await expect.element(path.getByRole('link', { name: 'accounts' })).toBeInTheDocument();
		await expect.element(path.getByText("Alice's Studio")).toBeInTheDocument();
		expect(currentStep()).toBe("Alice's Studio");
	});

	it('shows the root as ~, named "My Den" for screen readers', async () => {
		render(PathBar, { trail: deepTrail });

		const root = page.getByTestId('path-bar').getByRole('link', { name: 'My Den' });
		await expect.element(root).toHaveTextContent('~');
	});

	it('offers the parent as the back link on a phone', async () => {
		await page.viewport(320, 700);
		render(PathBar, { trail: deepTrail });

		const back = page.getByTestId('path-bar-back').getByRole('link');
		await expect.element(back).toHaveAttribute('href', '/accounts');
		await expect.element(back).toHaveTextContent('accounts');
	});

	it('has no back link when the page has no parent', async () => {
		const single: Trail = [
			{ step: 'named', label: 'Accounts', ordinal: undefined, href: undefined }
		];
		render(PathBar, { trail: single });

		await expect.element(page.getByTestId('path-bar-back')).not.toBeInTheDocument();
		await expect.element(page.getByText('Accounts')).toBeInTheDocument();
		expect(currentStep()).toBe('Accounts');
	});

	it('isolates a right-to-left name so it cannot reorder the bar', async () => {
		const rtl: Trail = [
			{ step: 'root', href: denHref('/den') },
			{ step: 'named', label: 'طلب رسم', ordinal: undefined, href: undefined }
		];
		render(PathBar, { trail: rtl });

		const name = page.getByTestId('path-bar').getByText('طلب رسم');
		await expect.element(name).toBeInTheDocument();
		expect(name.element().tagName).toBe('BDI');
	});

	it("shows each step's focus ring whole, uncut by the list", () => {
		render(PathBar, { trail: deepTrail });
		const list = page.getByTestId('path-bar').getByRole('list').element();
		const links = page.getByTestId('path-bar').getByRole('link').elements();
		expect(links).toHaveLength(2);

		for (const link of links) {
			link.focus();
			expect(link.matches(':focus-visible')).toBe(true);
			expect(parseFloat(getComputedStyle(link).outlineWidth)).toBeGreaterThan(0);

			const ring = ringBox(link);
			const visible = list.getBoundingClientRect();
			expect(ring.top).toBeGreaterThanOrEqual(visible.top);
			expect(ring.bottom).toBeLessThanOrEqual(visible.bottom);
			expect(ring.left).toBeGreaterThanOrEqual(visible.left);
			expect(ring.right).toBeLessThanOrEqual(visible.right);
		}
	});

	it('ends a long parent name in "…" on a phone, with no sideways scroll', async () => {
		await page.viewport(320, 700);
		render(PathBar, { trail: longParentTrail });

		const name = page
			.getByTestId('path-bar-back')
			.getByText(/^Supreme Arts/)
			.element();
		expect(name.tagName).toBe('BDI');
		expect(getComputedStyle(name).textOverflow).toBe('ellipsis');
		expect(name.scrollWidth).toBeGreaterThan(name.clientWidth);
		expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(window.innerWidth);
	});

	it('keeps the separator between steps out of what a screen reader says', async () => {
		render(PathBar, { trail: deepTrail });
		await expect
			.element(page.getByTestId('path-bar').getByRole('link', { name: 'accounts' }))
			.toBeVisible();

		const spoken = await spokenTexts();
		expect(spoken).toContain('accounts');
		expect(spoken.filter((text) => text.trim() === '/')).toEqual([]);
	});
});

describe('PathBar: equal names', () => {
	/** `~ / Test 1 (2) / Test 1 (3)`: a numbered parent link and a numbered current step. */
	const numberedTrail: Trail = [
		{ step: 'root', href: denHref('/den') },
		{ step: 'named', label: 'Test 1', ordinal: nameOrdinal(2), href: denHref('/den/c/b') },
		{ step: 'named', label: 'Test 1', ordinal: nameOrdinal(3), href: undefined }
	];

	it('shows each numbered step as "name (n)", the number outside the isolate', async () => {
		render(PathBar, { trail: numberedTrail });

		const path = page.getByTestId('path-bar');
		await expect.element(path.getByRole('link', { name: 'Test 1 (2)' })).toBeInTheDocument();
		expect(currentStep()).toBe('Test 1 (3)');
		const isolates = [...path.element().querySelectorAll('bdi')].map((bdi) => bdi.textContent);
		expect(isolates).toEqual(['Test 1', 'Test 1']);
	});

	it('shows the numbered parent as the back link on a phone', async () => {
		await page.viewport(320, 700);
		render(PathBar, { trail: numberedTrail });

		const back = page.getByTestId('path-bar-back').getByRole('link');
		await expect.element(back).toHaveTextContent('Test 1 (2)');
	});
});

describe('TrailStep links', () => {
	it('take only a Den link or a fixed section page, never an off-site address', () => {
		const offsite: Trail = [
			// @ts-expect-error -- a protocol-relative address must not type-check as a path step's link
			{ step: 'named', label: 'x', ordinal: undefined, href: '//evil.example/' }
		];
		expect(offsite).toHaveLength(1);
	});
});
