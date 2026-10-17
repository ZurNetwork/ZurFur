import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page, userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { TestStack } from '$lib/api/bug-report';
import type { Session } from '$lib/api/session';
import {
	devalued,
	kitCalls as kit,
	resetKitCalls,
	settleKitImports
} from '$lib/testing/kit-client';
import { denHref, did, handleFromTrusted } from '$lib/types/brand';
import { BUG_REPORT_KEY, BugReport } from './bug-report.svelte';
import ErrorView from './ErrorView.svelte';
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

/** The backend's reference: the last 8 characters of the report's id. */
const REFERENCE = '1a2b3c4d';
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

/**
 * Records every run of a form's `submit` listener (the one `enhance` adds),
 * so a spec can wait until `enhance` is done with an answer, its callback
 * included. Call before the form renders.
 */
function recordSubmits(): { settled: () => Promise<void> } {
	const runs: Promise<unknown>[] = [];
	vi.spyOn(HTMLFormElement.prototype, 'addEventListener').mockImplementation(function (
		this: HTMLFormElement,
		type: string,
		listener: EventListenerOrEventListenerObject,
		options?: boolean | AddEventListenerOptions
	) {
		if (type !== 'submit' || typeof listener !== 'function') {
			EventTarget.prototype.addEventListener.call(this, type, listener, options);
			return;
		}
		// `enhance`'s listener is async: read its run as the promise it returns.
		const handle: (this: HTMLFormElement, event: Event) => unknown = listener;
		const recorded = (event: Event): void => {
			runs.push(Promise.resolve(handle.call(this, event)));
		};
		EventTarget.prototype.addEventListener.call(this, type, recorded, options);
	});
	return {
		/** Every submit run finished, then the page's dynamic imports and one macrotask more for anything it left queued. */
		settled: async () => {
			expect(runs.length).toBeGreaterThan(0);
			await Promise.all(runs);
			await new Promise((done) => setTimeout(done, 0));
			await settleKitImports();
		}
	};
}

/** One frame's bug report and tree memory, shared by whatever renders in it. */
function frameContext(): {
	report: BugReport;
	memory: TreeMemory;
	context: Map<symbol, unknown>;
} {
	const report = new BugReport(alice.did);
	const memory = new TreeMemory(alice.did);
	memory.loaded(denHref('/den/accounts'), [], undefined);
	const context = new Map<symbol, unknown>([
		[BUG_REPORT_KEY, report],
		[TREE_MEMORY_KEY, memory]
	]);
	return { report, memory, context };
}

/** The dialog in `context`, on a stack of kind `testStack`. */
function renderDialog(context: Map<symbol, unknown>, testStack: TestStack): void {
	render(ReportBugDialog, {
		props: { session: alice, buildId: 'web-1.2.3', testStack },
		context
	} as never);
}

/** The top bar's link and the dialog, sharing one frame's bug report and tree memory. */
function renderFrame(testStack: TestStack = 'none'): { report: BugReport; memory: TreeMemory } {
	const { report, memory, context } = frameContext();
	render(ReportBugButton, { props: { current: false }, context } as never);
	renderDialog(context, testStack);
	return { report, memory };
}

/** The frame's error view for `status`, the top bar's link and the dialog. */
function renderErrorFrame(status: number): { report: BugReport } {
	const { report, context } = frameContext();
	render(ErrorView, {
		props: { status, routeId: '/(session)/den/[...path]', inFrame: true },
		context
	} as never);
	render(ReportBugButton, { props: { current: false }, context } as never);
	renderDialog(context, 'none');
	return { report };
}

const link = () => page.getByTestId('report-bug-link');
const dialog = () => page.getByRole('dialog', { name: 'Report a bug' });
const textBox = () => page.getByRole('textbox', { name: 'What happened?' });
const tick = (name: string) => page.getByRole('checkbox', { name, exact: true });
const sendButton = () => page.getByRole('button', { name: /^(Send report|Sending…)$/ });
const statusRegion = () => page.getByTestId('report-status');

const PAGE_NOTE = 'This address can contain the ids of your Accounts, characters and commissions.';
const DISPLAY_NOTE = 'Helps with display bugs. These settings can say something about your needs.';

/** The ids in `element`'s `aria-describedby`. */
function describedByIds(element: Element): readonly string[] {
	return (element.getAttribute('aria-describedby') ?? '').split(' ').filter((id) => id !== '');
}

/** Whether `inner` shows inside `outer`'s visible box. */
function showsInside(inner: Element, outer: Element): boolean {
	const innerBox = inner.getBoundingClientRect();
	const outerBox = outer.getBoundingClientRect();
	return innerBox.top >= outerBox.top && innerBox.bottom <= outerBox.bottom;
}

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
		await expect
			.element(tick('Page'))
			.toHaveAccessibleDescription(`Not sent · /den/x ${PAGE_NOTE}`);
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

	it('reads each privacy note with its tick box, after the value', async () => {
		renderFrame();
		await openDialog();

		await expect.element(tick('Page')).toHaveAccessibleDescription(`/den/x ${PAGE_NOTE}`);
		await expect
			.element(tick('Display settings'))
			.toHaveAccessibleDescription(
				/^Not sent · reduce motion (on|off) · more contrast (on|off) · forced colours (on|off) Helps with display bugs\. These settings can say something about your needs\.$/
			);
		await expect.element(page.getByText(PAGE_NOTE)).toBeVisible();
		await expect.element(page.getByText(DISPLAY_NOTE)).toBeVisible();
		const pageNoteId = page.getByText(PAGE_NOTE).element().id;
		expect(pageNoteId).not.toBe('');
		expect(describedByIds(tick('Page').element())).toContain(pageNoteId);
		await expect.element(tick('Page type')).toHaveAccessibleDescription('My Den');
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

		await expect
			.element(tick('Page'))
			.toHaveAccessibleDescription(`/den/x (showing archived) ${PAGE_NOTE}`);
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
		await expect.element(tick('Page')).toHaveAccessibleDescription(`/accounts ${PAGE_NOTE}`);
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

	it('reads the count with the text box while it shows', async () => {
		renderFrame();
		await openDialog();
		const hint = 'What did you do, what did you expect, and what happened instead?';

		await expect.element(textBox()).toHaveAccessibleDescription(hint);
		await textBox().fill('a'.repeat(3600));

		const count = page.getByTestId('report-count');
		await expect.element(count).toHaveTextContent('3,600 / 4,000');
		const countId = count.element().id;
		expect(countId).not.toBe('');
		expect(describedByIds(textBox().element())).toContain(countId);
		await expect.element(textBox()).toHaveAccessibleDescription(`${hint} 3,600 / 4,000`);

		await textBox().fill('a'.repeat(3599));
		await expect.element(count).not.toBeInTheDocument();
		await expect.element(textBox()).toHaveAccessibleDescription(hint);
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
	it('puts "Send report" before "Cancel" in the footer, in the order of the page', async () => {
		renderFrame();
		await openDialog();

		const send = sendButton().element();
		const cancel = page.getByRole('button', { name: 'Cancel' }).element();

		expect(send.compareDocumentPosition(cancel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		const sendLeft = send.getBoundingClientRect().left;
		const cancelLeft = cancel.getBoundingClientRect().left;
		expect(sendLeft).toBeLessThan(cancelLeft);
	});

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
		await expect
			.element(page.getByTestId('report-reference-line'))
			.toHaveTextContent(/^Reference: …1a2b3c4d$/);
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

describe('ReportBugDialog: a stack that keeps no reports', () => {
	it('says mock mode keeps no reports, in the form and still under the sent message', async () => {
		answerWith(sentResponse());
		renderFrame('mock');
		await openDialog();
		const line = page.getByTestId('report-test-stack');
		await expect.element(line).toHaveTextContent("Mock mode doesn't keep reports.");
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(page.getByText('Thanks. Your report is saved.')).toBeVisible();
		await expect.element(line).toBeVisible();
		await expect.element(line).toHaveTextContent("Mock mode doesn't keep reports.");
	});

	it('says the dev profile’s test stack forgets reports when it stops', async () => {
		renderFrame('devProfile');
		await openDialog();

		await expect
			.element(page.getByTestId('report-test-stack'))
			.toHaveTextContent('This test stack forgets reports when it stops.');
	});

	it('says nothing about the stack on a real one', async () => {
		renderFrame('none');
		await openDialog();

		await expect.element(page.getByTestId('report-test-stack')).not.toBeInTheDocument();
	});
});

describe('ReportBugDialog: opened from the error view', () => {
	const reportProblem = () => page.getByRole('link', { name: 'Report this problem' });

	it('carries the error’s status, ticked, and the page type "Error page"', async () => {
		renderErrorFrame(500);
		await expect.element(reportProblem()).toBeVisible();

		await reportProblem().click();

		await expect.element(textBox()).toHaveFocus();
		await expect.element(tick('Error')).toBeChecked();
		await expect.element(tick('Error')).toHaveAccessibleDescription('Status 500 on this page');
		await expect.element(tick('Page type')).toHaveAccessibleDescription('Error page');
	});

	it('posts the status while "Error" is ticked', async () => {
		const spy = answerWith(sentResponse());
		renderErrorFrame(500);
		await reportProblem().click();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.poll(() => spy.mock.calls.length).toBe(1);
		expect(posted(spy).get('sendErrorStatus')).toBe('on');
		expect(posted(spy).get('errorStatus')).toBe('500');
		expect(posted(spy).get('pageType')).toBe('Error page');
	});

	it('posts no status once "Error" is unticked', async () => {
		const spy = answerWith(sentResponse());
		renderErrorFrame(500);
		await reportProblem().click();
		await textBox().fill('It broke');
		await tick('Error').click();
		await expect
			.element(tick('Error'))
			.toHaveAccessibleDescription('Not sent · Status 500 on this page');

		await sendButton().click();

		await expect.poll(() => spy.mock.calls.length).toBe(1);
		expect(posted(spy).has('sendErrorStatus')).toBe(false);
		expect(posted(spy).has('errorStatus')).toBe(false);
	});

	it.each([399, 600, 302])(
		'offers no "Error" item for status %i, outside 400 to 599',
		async (status) => {
			renderErrorFrame(status);
			await reportProblem().click();

			await expect.element(textBox()).toHaveFocus();
			await expect.element(tick('Error')).not.toBeInTheDocument();
		}
	);

	it('gives focus back to "Report this problem" when the form closes', async () => {
		renderErrorFrame(404);
		await reportProblem().click();
		await expect.element(textBox()).toHaveFocus();

		await userEvent.keyboard('{Escape}');

		await expect.element(dialog()).not.toBeInTheDocument();
		await expect.element(reportProblem()).toHaveFocus();
	});

	it('leaves the status out when the top bar opens the form next', async () => {
		renderErrorFrame(500);
		await reportProblem().click();
		await expect.element(tick('Error')).toBeInTheDocument();
		await closeDialog();

		await page.getByRole('link', { name: 'Report a bug' }).click();

		await expect.element(textBox()).toHaveFocus();
		await expect.element(tick('Error')).not.toBeInTheDocument();
		await expect.element(tick('Page type')).toHaveAccessibleDescription('My Den');
	});
});

describe('ReportBugDialog: every answer the action gives', () => {
	it('success: the sent state, no invalidation, no applied result, the tree kept', async () => {
		answerWith(sentResponse());
		const submits = recordSubmits();
		const { memory } = renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(page.getByTestId('report-reference')).toHaveTextContent(REFERENCE);
		await submits.settled();
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
			name: 'pageAddress, with no box to paste into',
			data: { kind: 'pageAddress', ...echo },
			status: 400,
			text: "Page couldn't be sent. Untick Page to send."
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
			const submits = recordSubmits();
			renderFrame();
			await openDialog();
			await textBox().fill('It broke');

			await sendButton().click();

			await expect.element(statusRegion()).toHaveTextContent(text);
			await expect.element(statusRegion()).toHaveAttribute('role', 'status');
			await expect.element(textBox()).toHaveValue('It broke');
			await expect.element(sendButton()).toHaveTextContent('Send report');
			await submits.settled();
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

	const fieldCases: readonly {
		readonly name: string;
		readonly data: Readonly<Record<string, string>>;
		readonly text: string;
	}[] = [
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
	];

	it.each(fieldCases)(
		'failure $name: its message on "What happened?", focus moved there, the status region empty',
		async ({ data, text }) => {
			answerWith(failureResponse(data));
			renderFrame();
			await openDialog();
			await textBox().fill('It broke');

			await sendButton().click();

			const field = textBox();
			await expect.element(field).toHaveAttribute('aria-invalid', 'true');
			const message = dialog().getByText(text);
			await expect.element(message).toBeVisible();
			expect(describedByIds(field.element())).toContain(message.element().id);
			await expect.element(field).toHaveAccessibleDescription(new RegExp(`${text}$`));
			await expect.element(field).toHaveFocus();
			await expect.element(statusRegion()).toHaveTextContent('');
			await expect.element(field).toHaveValue('It broke');
		}
	);

	it.each([
		{ name: 'the limit (429)', data: { kind: 'limit', ...echo }, status: 429 },
		{ name: "couldn't send (502)", data: { kind: 'couldNotSend', ...echo }, status: 502 }
	])(
		'failure $name at 320 × 480: the message scrolled into the body’s view, focus left on "Send report"',
		async ({ data, status }) => {
			await page.viewport(320, 480);
			answerWith(failureResponse(data, status));
			renderFrame();
			await openDialog();
			await textBox().fill('It broke');
			const body = dialog().element().querySelector('.dialog__body');
			if (!(body instanceof HTMLElement)) throw new Error('the dialog has no body');
			body.scrollTop = 0;

			await sendButton().click();

			await expect.element(statusRegion()).not.toHaveTextContent('');
			const message = statusRegion().element();
			await expect.poll(() => showsInside(message, body)).toBe(true);
			expect(body.scrollTop).toBeGreaterThan(0);
			expect(document.activeElement).toBe(sendButton().element());
		}
	);

	it('error with status 503: "Couldn\'t send…", the text kept, the error never applied', async () => {
		answerWith(
			actionResponse(
				{ type: 'error', error: { message: "Zurfur isn't answering right now." } },
				503
			)
		);
		const submits = recordSubmits();
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(statusRegion()).toHaveTextContent(COULD_NOT_SEND);
		await expect.element(textBox()).toHaveValue('It broke');
		await submits.settled();
		expect(kit.applied).toEqual([]);
	});

	it('an unreadable 403: "Couldn\'t send…", the text kept', async () => {
		answerWith(new Response('Cross-site POST form submissions are forbidden', { status: 403 }));
		const submits = recordSubmits();
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.element(statusRegion()).toHaveTextContent(COULD_NOT_SEND);
		await expect.element(textBox()).toHaveValue('It broke');
		await submits.settled();
		expect(kit.applied).toEqual([]);
	});

	it('redirect: goes where it points', async () => {
		answerWith(actionResponse({ type: 'redirect', status: 303, location: '/login' }));
		const submits = recordSubmits();
		renderFrame();
		await openDialog();
		await textBox().fill('It broke');

		await sendButton().click();

		await expect.poll(() => kit.gotos).toEqual(['/login']);
		await submits.settled();
		expect(kit.applied).toEqual([]);
	});
});
