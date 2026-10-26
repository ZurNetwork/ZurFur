import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page, type Locator } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { BugReportOffer, TestStack } from '$lib/api/bug-report';
import {
	BUG_REPORT_KEY,
	BugReport,
	REPORT_PAGE_DESCRIPTION_ID
} from '$lib/components/explorer/bug-report.svelte';
import ReportBugButton from '$lib/components/explorer/ReportBugButton.svelte';
import {
	devalued,
	kitCalls as kit,
	resetKitCalls,
	settleKitImports
} from '$lib/testing/kit-client';
import { did, handleFromTrusted } from '$lib/types/brand';
import type { BugBrowser, BugPage } from '$lib/types/bug-report-rules';

vi.mock('$app/state', () => ({
	page: { url: new URL('http://localhost/report-a-bug'), data: {} }
}));

/** SvelteKit's client runtime, recording what the form asks of it (see the harness). */
vi.mock(
	'../../../../../node_modules/@sveltejs/kit/src/runtime/client/client.js',
	() => import('$lib/testing/kit-client')
);

const { default: ReportPage } = await import('./+page.svelte');

const session = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

const knownOffer: BugReportOffer = {
	page: { kind: 'known', page: '/den/commissions' as BugPage },
	pageType: 'My Den',
	browser: 'Mozilla/5.0 (X11)' as BugBrowser
};

const askOffer: BugReportOffer = { page: { kind: 'ask' }, pageType: undefined, browser: undefined };

function renderPage(offer: BugReportOffer, form?: unknown, testStack: TestStack = 'none'): void {
	render(ReportPage, {
		props: {
			data: { session, buildId: 'web-1.2.3', testStack, offer, trail: [] },
			form
		}
	} as never);
}

function answerWith(body: object, status = 200): MockInstance<typeof fetch> {
	return vi.spyOn(window, 'fetch').mockResolvedValue(Response.json(body, { status }));
}

function failWith(data: Readonly<Record<string, string>>, status = 400): void {
	answerWith({ type: 'failure', status, data: devalued(data) }, status);
}

const textBox = () => page.getByRole('textbox', { name: 'What happened?' });
const pageBox = () => page.getByRole('textbox', { name: 'Which page?' });
const tick = (name: string) => page.getByRole('checkbox', { name, exact: true });
const statusRegion = () => page.getByTestId('report-status');
const link = () => page.getByTestId('report-bug-link');

/** The ids in `element`'s `aria-describedby`. */
function describedByIds(element: Element): readonly string[] {
	return (element.getAttribute('aria-describedby') ?? '').split(' ').filter((id) => id !== '');
}

/**
 * Whether the page's own handlers left the last click to the browser
 * (recorded last, on the window, which then stops the navigation either way).
 */
const followed = { last: false };
function recordFollow(event: Event): void {
	followed.last = !event.defaultPrevented;
	event.preventDefault();
}

afterEach(() => {
	vi.restoreAllMocks();
	resetKitCalls();
});

/** Expect `field` invalid, described by `text` and focused, and the status region empty. */
async function expectOnField(field: Locator, text: string): Promise<void> {
	await expect.element(field).toHaveAttribute('aria-invalid', 'true');
	const message = page.getByText(text, { exact: true });
	await expect.element(message).toBeVisible();
	expect(describedByIds(field.element())).toContain(message.element().id);
	await expect.element(field).toHaveFocus();
	await expect.element(statusRegion()).toHaveTextContent('');
}

describe('/report-a-bug', () => {
	it('offers what the load read, and never the window or display settings', async () => {
		renderPage(knownOffer);

		await expect.element(page.getByRole('heading', { name: 'Report a bug' })).toBeInTheDocument();
		await expect
			.element(tick('Page'))
			.toHaveAccessibleDescription(
				'/den/commissions This address can contain the ids of your Accounts, characters and commissions.'
			);
		await expect.element(tick('Page type')).toHaveAccessibleDescription('My Den');
		await expect.element(tick('Zurfur version')).toHaveAccessibleDescription('web-1.2.3');
		await expect.element(tick('Browser')).toHaveAccessibleDescription('Mozilla/5.0 (X11)');
		await expect.element(tick('Window size')).not.toBeInTheDocument();
		await expect.element(tick('Display settings')).not.toBeInTheDocument();
		await expect
			.element(page.getByRole('textbox', { name: 'Which page?' }))
			.not.toBeInTheDocument();
		await expect
			.element(
				page.getByText(
					'Sent as @alice.zurfur.app. Zurfur also records when it arrives, and its own version.'
				)
			)
			.toBeInTheDocument();
	});

	it('gives its text box the id the top bar’s link focuses', async () => {
		renderPage(knownOffer);

		await expect.element(textBox()).toHaveAttribute('id', REPORT_PAGE_DESCRIPTION_ID);
		await expect.element(textBox()).not.toHaveAttribute('maxlength');
	});

	it('asks "Which page?" when it doesn’t know the page', async () => {
		renderPage(askOffer);

		await expect.element(page.getByRole('textbox', { name: 'Which page?' })).toBeInTheDocument();
		await expect.element(tick('Page')).not.toBeInTheDocument();
	});

	it('starts from a post answered without script: the reporter’s text, the page to change, the message', async () => {
		renderPage(knownOffer, { kind: 'pageAddress', description: 'Mine', page: 'not a page' });

		await expect.element(textBox()).toHaveValue('Mine');
		await expect.element(pageBox()).toHaveValue('not a page');
		await expectOnField(pageBox(), "Paste the page's address, or leave this empty.");
		await expect.element(textBox()).not.toHaveAttribute('aria-invalid');
	});

	it('shows the reference a post without script answered', async () => {
		renderPage(knownOffer, { kind: 'sent', reference: '9f8e7d6c' });

		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeInTheDocument();
		await expect.element(page.getByTestId('report-reference')).toHaveTextContent('9f8e7d6c');
		await expect
			.element(page.getByTestId('report-reference-line'))
			.toHaveTextContent(/^Reference: …9f8e7d6c$/);
	});

	it('says mock mode keeps no reports, still under the reference a post without script answered', async () => {
		renderPage(knownOffer, { kind: 'sent', reference: '9f8e7d6c' }, 'mock');

		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeVisible();
		await expect
			.element(page.getByTestId('report-test-stack'))
			.toHaveTextContent("Mock mode doesn't keep reports.");
	});

	it('says the dev profile’s test stack forgets reports, in the form’s fixed lines', async () => {
		renderPage(knownOffer, undefined, 'devProfile');

		await expect
			.element(page.getByTestId('report-test-stack'))
			.toHaveTextContent('This test stack forgets reports when it stops.');
	});

	it('posts in place with script, the shared field names, and shows the reference', async () => {
		const spy = answerWith({
			type: 'success',
			status: 200,
			data: devalued({ kind: 'sent', reference: 'BR-10' })
		});
		renderPage(knownOffer);
		await textBox().fill('It broke');

		await page.getByRole('button', { name: 'Send report' }).click();

		await expect.element(page.getByTestId('report-announcer')).toHaveTextContent('Report sent.');
		await expect.element(page.getByTestId('report-reference')).toHaveTextContent('BR-10');
		const fields = new URLSearchParams(spy.mock.calls[0]?.[1]?.body as URLSearchParams);
		expect(fields.get('description')).toBe('It broke');
		expect(fields.get('page')).toBe('/den/commissions');
		expect(fields.get('browser')).toBe('Mozilla/5.0 (X11)');
		await settleKitImports();
		expect(kit.applied).toEqual([]);
		expect(kit.invalidations).toBe(0);
	});

	it('keeps the text when the limit is reached', async () => {
		answerWith(
			{
				type: 'failure',
				status: 429,
				data: devalued({ kind: 'limit', description: 'It broke', page: '' })
			},
			429
		);
		renderPage(knownOffer);
		await textBox().fill('It broke');

		await page.getByRole('button', { name: 'Send report' }).click();

		await expect
			.element(page.getByTestId('report-status'))
			.toHaveTextContent(
				"You've sent several reports in the last hour. Please try again a little later."
			);
		await expect.element(textBox()).toHaveValue('It broke');
	});
});

describe('/report-a-bug: a field’s message on its field', () => {
	const echo = { description: 'It broke', page: 'den page' };

	it.each([
		{
			name: 'descriptionTooLong',
			data: { kind: 'descriptionTooLong', ...echo },
			text: 'Your text is longer than 4,000 characters. Please shorten it.'
		},
		{
			name: 'hiddenCharacter in the description',
			data: { kind: 'hiddenCharacter', item: 'description', ...echo },
			text: 'Your text has a hidden formatting character. Please remove it.'
		}
	])('$name: on "What happened?", focus moved there', async ({ data, text }) => {
		failWith(data);
		renderPage(askOffer);
		await textBox().fill('It broke');

		await page.getByRole('button', { name: 'Send report' }).click();

		await expectOnField(textBox(), text);
		await expect.element(pageBox()).not.toHaveAttribute('aria-invalid');
	});

	it.each([
		{
			name: 'pageAddress',
			data: { kind: 'pageAddress', ...echo },
			text: "Paste the page's address, or leave this empty."
		},
		{
			name: 'hiddenCharacter in the page',
			data: { kind: 'hiddenCharacter', item: 'page', ...echo },
			text: "Your page's address has a hidden character. Please remove it."
		}
	])('$name: on "Which page?", focus moved there', async ({ data, text }) => {
		failWith(data);
		renderPage(askOffer);
		await textBox().fill('It broke');
		await pageBox().fill('den page');

		await page.getByRole('button', { name: 'Send report' }).click();

		await expectOnField(pageBox(), text);
		await expect.element(textBox()).not.toHaveAttribute('aria-invalid');
	});

	it('a refused page it knew: the tick box’s wording in the status region', async () => {
		failWith({ kind: 'pageAddress', ...echo });
		renderPage(knownOffer);
		await textBox().fill('It broke');

		await page.getByRole('button', { name: 'Send report' }).click();

		await expect
			.element(statusRegion())
			.toHaveTextContent("Page couldn't be sent. Untick Page to send.");
		await expect.element(textBox()).not.toHaveAttribute('aria-invalid');
	});
});

describe('/report-a-bug: "Report a bug" on the page itself', () => {
	beforeEach(() => {
		window.addEventListener('click', recordFollow);
	});

	afterEach(() => {
		window.removeEventListener('click', recordFollow);
	});

	/** The page answered `form`, beside the top bar's link in the same frame. */
	function renderWithLink(form?: unknown): void {
		renderPage(knownOffer, form);
		render(ReportBugButton, {
			props: { current: true },
			context: new Map([[BUG_REPORT_KEY, new BugReport(session.did)]])
		} as never);
	}

	it('focuses "What happened?" while the form is there, the click kept', async () => {
		renderWithLink();
		await expect.element(textBox()).toBeInTheDocument();

		await link().click();

		await expect.element(textBox()).toHaveFocus();
		expect(followed.last).toBe(false);
	});

	it('after a send, loads the page afresh: the click left to the browser, for a full load', async () => {
		renderWithLink({ kind: 'sent', reference: '3f9a1c2e' });
		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeInTheDocument();
		await expect.element(textBox()).not.toBeInTheDocument();

		await link().click();

		expect(followed.last).toBe(true);
		await expect.element(link()).toHaveAttribute('href', '/report-a-bug');
		await expect.element(link()).toHaveAttribute('data-sveltekit-reload');
	});
});
