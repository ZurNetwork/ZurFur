import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { Session } from '$lib/api/session';
import { did, handleFromTrusted } from '$lib/types/brand';
import {
	BUG_REPORT_KEY,
	BugReport,
	REPORT_PAGE_DESCRIPTION_ID,
	snapshotOf
} from './bug-report.svelte';
import type { FrameSection } from './frame-section';
import ReportBugButton from './ReportBugButton.svelte';
import TopBar from './TopBar.svelte';

const alice: Session = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

/** The top bar inside a frame whose bug report always reads the same snapshot. */
function renderTopBar(section: FrameSection): BugReport {
	const report = new BugReport(alice.did, (extras) =>
		snapshotOf({
			pathname: '/den',
			showingArchived: false,
			userAgent: 'Mozilla/5.0',
			width: 1280,
			height: 800,
			display: { reducedMotion: false, moreContrast: false, forcedColors: false },
			extras
		})
	);
	render(TopBar, {
		props: {
			session: alice,
			trail: [],
			section,
			drawerOpen: false,
			drawerId: 'drawer',
			onOpenDrawer: () => undefined
		},
		context: new Map([[BUG_REPORT_KEY, report]])
	} as never);
	return report;
}

/**
 * Whether the page's own handler kept the browser from following the link
 * (recorded last, on the window, which then stops the navigation either way).
 */
const followed = { last: false };
function recordFollow(event: Event): void {
	followed.last = !event.defaultPrevented;
	event.preventDefault();
}

const link = () => page.getByTestId('report-bug-link');
const added: HTMLElement[] = [];

beforeEach(async () => {
	await page.viewport(1280, 800);
	window.addEventListener('click', recordFollow);
});

afterEach(() => {
	window.removeEventListener('click', recordFollow);
	for (const element of added.splice(0)) element.remove();
});

describe('ReportBugButton in the top bar', () => {
	it('is a link to the report page that says it opens a dialog once the script runs', async () => {
		renderTopBar('den');

		await expect.element(link()).toHaveAttribute('href', '/report-a-bug');
		await expect.element(link()).toHaveAttribute('aria-haspopup', 'dialog');
		await expect.element(link()).toHaveAccessibleName('Report a bug');
		await expect.element(link()).toBeVisible();
	});

	it('sits in the banner, outside the "Sections" navigation', () => {
		renderTopBar('den');

		const sections = page.getByRole('navigation', { name: 'Sections' }).element();
		const banner = page.getByRole('banner').element();
		expect(banner.contains(link().element())).toBe(true);
		expect(sections.contains(link().element())).toBe(false);
	});

	it('opens the dialog on a plain click, recording the link as the opener', async () => {
		const report = renderTopBar('den');

		await link().click();

		expect(report.isOpen).toBe(true);
		expect(report.opener).toBe(link().element());
		expect(followed.last).toBe(false);
	});

	it('follows the link on a modified click', () => {
		const report = renderTopBar('den');

		for (const modifier of ['ctrlKey', 'metaKey', 'shiftKey'] as const) {
			link()
				.element()
				.dispatchEvent(
					new MouseEvent('click', { bubbles: true, cancelable: true, [modifier]: true })
				);
			expect(followed.last).toBe(true);
		}
		expect(report.isOpen).toBe(false);
	});

	it('on the report page, is marked current and focuses the page’s text box', async () => {
		const box = document.createElement('textarea');
		box.id = REPORT_PAGE_DESCRIPTION_ID;
		document.body.append(box);
		added.push(box);
		const report = renderTopBar('report');

		await expect.element(link()).toHaveAttribute('aria-current', 'page');
		await link().click();

		expect(document.activeElement).toBe(box);
		expect(report.isOpen).toBe(false);
		expect(followed.last).toBe(false);
	});

	it('on the report page with no text box (a report just sent), leaves the click to the browser for a full load', async () => {
		const report = renderTopBar('report');

		await link().click();

		expect(followed.last).toBe(true);
		await expect.element(link()).toHaveAttribute('data-sveltekit-reload');
		expect(report.isOpen).toBe(false);
	});

	it('elsewhere, never asks for a full load', async () => {
		renderTopBar('den');

		await expect.element(link()).not.toHaveAttribute('data-sveltekit-reload');
	});

	it('is not in the top bar on a phone', async () => {
		await page.viewport(320, 640);
		renderTopBar('den');

		await expect.element(link()).not.toBeVisible();
	});
});

describe('ReportBugButton in the drawer, on the report page', () => {
	/** The drawer's link, with a `closeFirst` that records it ran and answers ☰. */
	function renderDrawerLink(): { closes: () => number } {
		const menu = document.createElement('button');
		document.body.append(menu);
		added.push(menu);
		const counter = { runs: 0 };
		render(ReportBugButton, {
			props: {
				current: true,
				closeFirst: () => {
					counter.runs += 1;
					return Promise.resolve(menu);
				}
			},
			context: new Map([[BUG_REPORT_KEY, new BugReport(alice.did)]])
		} as never);
		return { closes: () => counter.runs };
	}

	it('closes the drawer, then focuses the page’s text box, the click kept', async () => {
		const box = document.createElement('textarea');
		box.id = REPORT_PAGE_DESCRIPTION_ID;
		document.body.append(box);
		added.push(box);
		const drawer = renderDrawerLink();

		await link().click();

		expect(followed.last).toBe(false);
		expect(drawer.closes()).toBe(1);
		await expect.poll(() => document.activeElement).toBe(box);
	});

	it('with no text box (a report just sent), closes the drawer and leaves the click to the browser', async () => {
		const drawer = renderDrawerLink();

		await link().click();

		expect(followed.last).toBe(true);
		expect(drawer.closes()).toBe(1);
		await expect.element(link()).toHaveAttribute('data-sveltekit-reload');
	});
});
