import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { BugReportOffer } from '$lib/api/bug-report';
import { REPORT_PAGE_DESCRIPTION_ID } from '$lib/components/explorer/bug-report.svelte';
import { devalued, kitCalls as kit, resetKitCalls } from '$lib/testing/kit-client';
import { did, handleFromTrusted } from '$lib/types/brand';
import type { BugBrowser, BugPage } from '$lib/types/bug-report-rules';

vi.mock('$app/state', () => ({
	page: { url: new URL('http://localhost/report-a-bug'), data: {} }
}));

/** SvelteKit's client runtime, recording what the form asks of it (see the harness). */
vi.mock(
	'../../../../node_modules/@sveltejs/kit/src/runtime/client/client.js',
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

function renderPage(offer: BugReportOffer, form?: unknown): void {
	render(ReportPage, {
		props: {
			data: { session, buildId: 'web-1.2.3', offer, trail: [] },
			form
		}
	} as never);
}

function answerWith(body: object, status = 200): MockInstance<typeof fetch> {
	return vi.spyOn(window, 'fetch').mockResolvedValue(Response.json(body, { status }));
}

const textBox = () => page.getByRole('textbox', { name: 'What happened?' });
const tick = (name: string) => page.getByRole('checkbox', { name, exact: true });

afterEach(() => {
	vi.restoreAllMocks();
	resetKitCalls();
});

describe('/report-a-bug', () => {
	it('offers what the load read, and never the window or display settings', async () => {
		renderPage(knownOffer);

		await expect.element(page.getByRole('heading', { name: 'Report a bug' })).toBeInTheDocument();
		await expect.element(tick('Page')).toHaveAccessibleDescription('/den/commissions');
		await expect.element(tick('Page type')).toHaveAccessibleDescription('My Den');
		await expect.element(tick('Zurfur version')).toHaveAccessibleDescription('web-1.2.3');
		await expect.element(tick('Browser')).toHaveAccessibleDescription('Mozilla/5.0 (X11)');
		await expect.element(tick('Window size')).not.toBeInTheDocument();
		await expect.element(tick('Display settings')).not.toBeInTheDocument();
		await expect
			.element(page.getByRole('textbox', { name: 'Which page?' }))
			.not.toBeInTheDocument();
		await expect
			.element(page.getByText('Sent as @alice.zurfur.app. Zurfur also records when it arrives.'))
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
		await expect
			.element(page.getByRole('textbox', { name: 'Which page?' }))
			.toHaveValue('not a page');
		await expect
			.element(page.getByTestId('report-status'))
			.toHaveTextContent("Paste the page's address, or leave this empty.");
	});

	it('shows the reference a post without script answered', async () => {
		renderPage(knownOffer, { kind: 'sent', reference: 'BR-9' });

		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeInTheDocument();
		await expect.element(page.getByTestId('report-reference')).toHaveTextContent('BR-9');
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
