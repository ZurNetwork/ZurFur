import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page, userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { Session } from '$lib/api/session';
import { devalued, kitCalls as kit, resetKitCalls } from '$lib/testing/kit-client';
import { denHref, did, handleFromTrusted } from '$lib/types/brand';
import { BUG_REPORT_KEY, BugReport } from './bug-report.svelte';
import ReportBugButton from './ReportBugButton.svelte';
import ReportBugDialog from './ReportBugDialog.svelte';
import { TREE_MEMORY_KEY, TreeMemory } from './tree-memory.svelte';

/** The open page, as `$app/state` reports it. */
const open = vi.hoisted(() => ({
	url: new URL('http://localhost/den/x'),
	data: {}
}));
vi.mock('$app/state', () => ({
	page: {
		get url() {
			return open.url;
		},
		get data() {
			return open.data;
		}
	}
}));

/** SvelteKit's client runtime, recording what the form asks of it (see the harness). */
vi.mock(
	'../../../../node_modules/@sveltejs/kit/src/runtime/client/client.js',
	() => import('$lib/testing/kit-client')
);

const alice: Session = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

const REFERENCE = 'BR-7Q2X';
const COULD_NOT_SEND = "Couldn't send your report. Your text is still here; please try again.";

function actionResponse(body: object, status = 200): Response {
	return Response.json(body, { status });
}

function sentResponse(): Response {
	return actionResponse({
		type: 'success',
		status: 200,
		data: devalued({ kind: 'sent', reference: REFERENCE })
	});
}

function failureResponse(data: Readonly<Record<string, string>>, status = 400): Response {
	return actionResponse({ type: 'failure', status, data: devalued(data) }, status);
}

/** The action answering every post with `response`. */
function answerWith(response: Response): MockInstance<typeof fetch> {
	return vi.spyOn(window, 'fetch').mockResolvedValue(response);
}

/** The action holding its answer until the test gives it. */
function answerLater(): {
	fetch: MockInstance<typeof fetch>;
	answer: (response: Response) => void;
} {
	const held: { answer: (response: Response) => void } = { answer: () => undefined };
	const spy = vi.spyOn(window, 'fetch').mockReturnValue(
		new Promise<Response>((settle) => {
			held.answer = settle;
		})
	);
	return {
		fetch: spy,
		answer: (response) => {
			held.answer(response);
		}
	};
}

/** The fields the `index`th post sent. */
function posted(spy: MockInstance<typeof fetch>, index = 0): URLSearchParams {
	const init = spy.mock.calls[index]?.[1];
	return new URLSearchParams(init?.body as URLSearchParams);
}

/** The top bar's link and the dialog, sharing one frame's bug report and tree memory. */
function renderFrame(): { report: BugReport; memory: TreeMemory } {
	const report = new BugReport(alice.did);
	const memory = new TreeMemory(alice.did);
	memory.loaded(denHref('/den/accounts'), [], undefined);
	const context = new Map<symbol, unknown>([
		[BUG_REPORT_KEY, report],
		[TREE_MEMORY_KEY, memory]
	]);
	render(ReportBugButton, { props: { current: false }, context } as never);
	render(ReportBugDialog, { props: { session: alice, buildId: 'web-1.2.3' }, context } as never);
	return { report, memory };
}

const link = () => page.getByTestId('report-bug-link');
const dialog = () => page.getByRole('dialog', { name: 'Report a bug' });
const textBox = () => page.getByRole('textbox', { name: 'What happened?' });
const tick = (name: string) => page.getByRole('checkbox', { name, exact: true });
const sendButton = () => page.getByRole('button', { name: /^(Send report|Sending…)$/ });
const statusRegion = () => page.getByTestId('report-status');

async function openDialog(): Promise<void> {
	await link().click();
	await expect.element(textBox()).toHaveFocus();
}

async function closeDialog(): Promise<void> {
	await page.getByRole('button', { name: 'Cancel' }).click();
	await expect.element(dialog()).not.toBeInTheDocument();
}

/** Leave focus on the body, as Safari does after a click on a link. */
function focusNothing(): void {
	const active = document.activeElement;
	if (active instanceof HTMLElement) active.blur();
	expect(document.activeElement).toBe(document.body);
}

beforeEach(async () => {
	await page.viewport(1280, 800);
	open.url = new URL('http://localhost/den/x');
	open.data = {};
});

afterEach(() => {
	vi.restoreAllMocks();
	resetKitCalls();
	Reflect.deleteProperty(navigator, 'userAgent');
});

describe('ReportBugDialog: what is sent with the report', () => {
	it('display_settings_start_unticked_and_say_not_sent', async () => {
		renderFrame();
		await openDialog();

		const display = tick('Display settings');
		await expect.element(display).not.toBeChecked();
		await expect.element(display).toHaveAccessibleDescription(/^Not sent · reduce motion (on|off)/);
		for (const name of ['Page', 'Page type', 'Zurfur version', 'Browser', 'Window size']) {
			await expect.element(tick(name)).toBeChecked();
		}
	});

	it('ticks_survive_close_and_reopen', async () => {
		renderFrame();
		await openDialog();
		await tick('Page').click();
		await closeDialog();

		await openDialog();

		await expect.element(tick('Page')).not.toBeChecked();
		await expect.element(tick('Page')).toHaveAccessibleDescription('Not sent · /den/x');
	});

	it('ticks_reset_after_a_send', async () => {
		answerWith(sentResponse());
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');
		await tick('Page').click();
		await tick('Display settings').click();
		await sendButton().click();
		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeInTheDocument();
		await page.getByRole('button', { name: 'Close', exact: true }).last().click();

		await openDialog();

		await expect.element(tick('Page')).toBeChecked();
		await expect.element(tick('Display settings')).not.toBeChecked();
		await expect.element(textBox()).toHaveValue('');
	});

	it('is a group named "Sent with your report", each box described by its value', async () => {
		renderFrame();
		await openDialog();

		const group = page.getByRole('group', { name: 'Sent with your report' });
		await expect.element(group).toBeInTheDocument();
		await expect
			.element(group.getByRole('checkbox', { name: 'Page type' }))
			.toHaveAccessibleDescription('My Den');
		await expect
			.element(group.getByRole('checkbox', { name: 'Zurfur version' }))
			.toHaveAccessibleDescription('web-1.2.3');
		await expect
			.element(group.getByRole('checkbox', { name: 'Window size' }))
			.toHaveAccessibleDescription(`${String(innerWidth)} × ${String(innerHeight)}`);
		await expect
			.element(group.getByRole('checkbox', { name: 'Browser' }))
			.toHaveAccessibleDescription(navigator.userAgent);
	});

	it('posts only the ticked items, under the shared field names', async () => {
		const spy = answerWith(sentResponse());
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');
		await tick('Browser').click();
		await sendButton().click();

		await expect.poll(() => spy.mock.calls.length).toBe(1);
		const fields = posted(spy);
		const target = spy.mock.calls[0]?.[0];
		expect(target instanceof URL ? target.pathname : undefined).toBe('/report-a-bug');
		expect(fields.get('description')).toBe('It broke');
		expect(fields.get('sendPage')).toBe('on');
		expect(fields.get('page')).toBe('/den/x');
		expect(fields.get('pageType')).toBe('My Den');
		expect(fields.get('sendWebVersion')).toBe('on');
		expect(fields.get('windowWidth')).toBe(String(innerWidth));
		expect(fields.has('sendBrowser')).toBe(false);
		expect(fields.has('browser')).toBe(false);
		expect(fields.has('sendDisplay')).toBe(false);
		expect(fields.has('reducedMotion')).toBe(false);
	});
});

describe('ReportBugDialog: the page it reports', () => {
	it('shows the path with the archived words, never the query, and posts exactly that', async () => {
		open.url = new URL('http://localhost/den/x?includeDeleted=true&pageToken=abc');
		open.data = { den: { includeDeleted: true } };
		const spy = answerWith(sentResponse());
		renderFrame();
		await openDialog();

		await expect.element(tick('Page')).toHaveAccessibleDescription('/den/x (showing archived)');
		await expect.element(dialog().getByText(/pageToken|includeDeleted/)).not.toBeInTheDocument();
		await textBox().fill('It broke');
		await sendButton().click();

		await expect.poll(() => spy.mock.calls.length).toBe(1);
		expect(posted(spy).get('page')).toBe('/den/x (showing archived)');
	});

	it('reads the page and the window again at each open, keeping the text and ticks', async () => {
		const { report } = renderFrame();
		await openDialog();
		await textBox().fill('Kept');
		await tick('Browser').click();
		await closeDialog();

		open.url = new URL('http://localhost/accounts');
		await page.viewport(1000, 700);
		report.open(link().element() as HTMLElement);
		await expect.element(textBox()).toHaveFocus();

		await expect.element(textBox()).toHaveValue('Kept');
		await expect.element(tick('Browser')).not.toBeChecked();
		await expect.element(tick('Page')).toHaveAccessibleDescription('/accounts');
		await expect.element(tick('Page type')).toHaveAccessibleDescription('Accounts');
		await expect.element(tick('Window size')).toHaveAccessibleDescription('1000 × 700');
	});
});

describe('ReportBugDialog: "What happened?"', () => {
	it('never caps the text box, and counts code points near the limit', async () => {
		renderFrame();
		await openDialog();

		await expect.element(textBox()).not.toHaveAttribute('maxlength');
		await textBox().fill('a'.repeat(3599));
		await expect.element(page.getByTestId('report-count')).not.toBeInTheDocument();
		await textBox().fill('😀'.repeat(3600));
		await expect.element(page.getByTestId('report-count')).toHaveTextContent('3,600 / 4,000');
	});

	it('keeps the draft when closed, and gives focus back to the link', async () => {
		renderFrame();
		await openDialog();
		await textBox().fill('Half a thought');
		await userEvent.keyboard('{Escape}');
		await expect.element(dialog()).not.toBeInTheDocument();
		await expect.element(link()).toHaveFocus();

		await openDialog();

		await expect.element(textBox()).toHaveValue('Half a thought');
	});

	it('gives focus back to the clicked link even when the click left focus on the body', async () => {
		renderFrame();
		focusNothing();
		const anchor = link().element() as HTMLElement;
		anchor.click();
		await expect.element(textBox()).toHaveFocus();

		await userEvent.keyboard('{Escape}');

		await expect.element(dialog()).not.toBeInTheDocument();
		expect(document.activeElement).toBe(anchor);
	});
});

describe('ReportBugDialog: sending', () => {
	it('has an empty polite live region before sending, then says "Report sent." and shows the reference', async () => {
		answerWith(sentResponse());
		renderFrame();
		await openDialog();
		const announcer = page.getByTestId('report-announcer');
		await expect.element(announcer).toHaveAttribute('aria-live', 'polite');
		await expect.element(announcer).toHaveTextContent('');
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(announcer).toHaveTextContent('Report sent.');
		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeInTheDocument();
		await expect.element(page.getByTestId('report-reference')).toHaveTextContent(REFERENCE);
		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeVisible();
		expect(document.activeElement?.textContent).toContain('Thanks. Your report is saved.');
	});

	it('marks "Send report" busy while sending, and a second press sends nothing', async () => {
		const held = answerLater();
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();
		await expect.element(sendButton()).toHaveTextContent('Sending…');
		await expect.element(sendButton()).toHaveAttribute('aria-disabled', 'true');
		await sendButton().click({ force: true });

		expect(held.fetch).toHaveBeenCalledTimes(1);
		held.answer(sentResponse());
		await expect.element(page.getByTestId('report-reference')).toHaveTextContent(REFERENCE);
		expect(held.fetch).toHaveBeenCalledTimes(1);
	});

	it('shows the reference on reopen when the dialog closed mid-send', async () => {
		const held = answerLater();
		const { report } = renderFrame();
		await openDialog();
		await textBox().fill('It broke');
		await sendButton().click();
		await expect.element(sendButton()).toHaveTextContent('Sending…');
		await userEvent.keyboard('{Escape}');
		await expect.element(dialog()).not.toBeInTheDocument();

		held.answer(sentResponse());
		await expect.poll(() => report.draft.status.status).toBe('sent');
		await link().click();

		await expect.element(page.getByTestId('report-reference')).toHaveTextContent(REFERENCE);
	});

	it('fills a 320 × 480 screen with a long browser text, "Send report" in view, nothing sideways', async () => {
		Object.defineProperty(navigator, 'userAgent', {
			configurable: true,
			get: () => `Mozilla/5.0 ${'X'.repeat(600)}`
		});
		await page.viewport(320, 480);
		const { report } = renderFrame();
		report.open(document.body);
		await expect.element(textBox()).toHaveFocus();

		const box = dialog().element().getBoundingClientRect();
		expect(box.left).toBe(0);
		expect(box.top).toBe(0);
		expect(box.width).toBe(innerWidth);
		expect(box.height).toBe(innerHeight);
		await expect.element(textBox()).toHaveAttribute('rows', '6');
		await expect.element(tick('Browser')).toHaveAccessibleDescription(/…$/);

		await expect.element(sendButton()).toBeVisible();
		const send = sendButton().element().getBoundingClientRect();
		expect(send.bottom).toBeLessThanOrEqual(innerHeight);
		expect(send.right).toBeLessThanOrEqual(innerWidth);
		const body = dialog().element().querySelector('.dialog__body');
		if (!(body instanceof HTMLElement)) throw new Error('the dialog has no body');
		expect(body.scrollWidth).toBeLessThanOrEqual(body.clientWidth);
		expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(innerWidth);
	});
});

describe('ReportBugDialog: every answer the action gives', () => {
	it('success: the sent state, no invalidation, no applied result, the tree kept', async () => {
		answerWith(sentResponse());
		const { memory } = renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(page.getByTestId('report-reference')).toHaveTextContent(REFERENCE);
		expect(kit.invalidations).toBe(0);
		expect(kit.applied).toEqual([]);
		expect(kit.gotos).toEqual([]);
		expect(memory.folders.has(denHref('/den/accounts'))).toBe(true);
	});

	const echo = { description: 'It broke', page: '/den/x' };
	const statusCases: readonly {
		readonly name: string;
		readonly data: Readonly<Record<string, string>>;
		readonly status: number;
		readonly text: string;
	}[] = [
		{
			name: 'descriptionTooLong',
			data: { kind: 'descriptionTooLong', ...echo },
			status: 400,
			text: 'Your text is longer than 4,000 characters. Please shorten it.'
		},
		{
			name: 'hiddenCharacter in the description',
			data: { kind: 'hiddenCharacter', item: 'description', ...echo },
			status: 400,
			text: 'Your text has a hidden formatting character. Please remove it.'
		},
		{
			name: 'hiddenCharacter in the browser',
			data: { kind: 'hiddenCharacter', item: 'browser', ...echo },
			status: 400,
			text: "Your browser's text has a hidden character. Untick Browser to send."
		},
		{
			name: 'hiddenCharacter in the page',
			data: { kind: 'hiddenCharacter', item: 'page', ...echo },
			status: 400,
			text: "Your page's address has a hidden character. Untick Page to send."
		},
		{
			name: 'pageAddress',
			data: { kind: 'pageAddress', ...echo },
			status: 400,
			text: "Paste the page's address, or leave this empty."
		},
		{
			name: 'itemRefused',
			data: { kind: 'itemRefused', item: 'window', ...echo },
			status: 400,
			text: "Window size couldn't be sent. Untick Window size to send."
		},
		{
			name: 'limit',
			data: { kind: 'limit', ...echo },
			status: 429,
			text: "You've sent several reports in the last hour. Please try again a little later."
		},
		{
			name: 'couldNotSend',
			data: { kind: 'couldNotSend', ...echo },
			status: 502,
			text: COULD_NOT_SEND
		}
	];

	it.each(statusCases)(
		'failure $name: its message in the status region, the text kept',
		async ({ data, status, text }) => {
			answerWith(failureResponse(data, status));
			renderFrame();
			await openDialog();
			await textBox().fill('It broke');

			await sendButton().click();

			await expect.element(statusRegion()).toHaveTextContent(text);
			await expect.element(statusRegion()).toHaveAttribute('role', 'status');
			await expect.element(textBox()).toHaveValue('It broke');
			await expect.element(sendButton()).toHaveTextContent('Send report');
			expect(kit.applied).toEqual([]);
		}
	);

	it('failure descriptionEmpty: "Tell us what happened." on the field, focus moved there', async () => {
		answerWith(failureResponse({ kind: 'descriptionEmpty', description: '', page: '/den/x' }));
		renderFrame();
		await openDialog();
		await tick('Page').click();

		await sendButton().click();

		const field = textBox();
		await expect.element(field).toHaveAttribute('aria-invalid', 'true');
		await expect.element(field).toHaveAccessibleDescription(/Tell us what happened\./);
		await expect.element(field).toHaveFocus();
		await expect.element(tick('Page')).not.toBeChecked();
	});

	it('error with status 503: "Couldn\'t send…", the text kept, the error never applied', async () => {
		answerWith(
			actionResponse(
				{ type: 'error', error: { message: "Zurfur isn't answering right now." } },
				503
			)
		);
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(statusRegion()).toHaveTextContent(COULD_NOT_SEND);
		await expect.element(textBox()).toHaveValue('It broke');
		expect(kit.applied).toEqual([]);
	});

	it('an unreadable 403: "Couldn\'t send…", the text kept', async () => {
		answerWith(new Response('Cross-site POST form submissions are forbidden', { status: 403 }));
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(statusRegion()).toHaveTextContent(COULD_NOT_SEND);
		await expect.element(textBox()).toHaveValue('It broke');
		expect(kit.applied).toEqual([]);
	});

	it('redirect: goes where it points', async () => {
		answerWith(actionResponse({ type: 'redirect', status: 303, location: '/login' }));
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.poll(() => kit.gotos).toEqual(['/login']);
		expect(kit.applied).toEqual([]);
	});
});
