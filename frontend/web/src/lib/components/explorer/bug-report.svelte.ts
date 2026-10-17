/**
 * Reporting a bug from the signed-in frame: the draft (the reporter's text,
 * the tick boxes and the send in flight), what the dialog shows as sent with
 * the report, and how an action's answer settles the draft. The frame owns
 * one {@link BugReport} (never module-level, so it is never shared between
 * visitors); the report page keeps a {@link BugReportDraft} of its own.
 */

import type { ActionResult } from '@sveltejs/kit';
import { getContext, setContext } from 'svelte';
import { page } from '$app/state';
import {
	ARCHIVED_SUFFIX,
	BUG_REPORT_FIELDS,
	DISPLAY_MATCHED,
	DISPLAY_UNMATCHED,
	isBugReportActionData,
	type BugReportActionData,
	type BugReportFailure,
	type BugReportFieldName,
	type BugReportItem,
	type BugReportOffer,
	type BugReportRefusalKind,
	type TestStack
} from '$lib/api/bug-report';
import { isRenderableErrorStatus } from '$lib/api/problem';
import type { BugReference, Did } from '$lib/types/brand';
import { cutBrowser, pageTypeOf, type PageType } from '$lib/types/bug-report-rules';

/** The id of the report page's "What happened?" box, which the top bar's link focuses there. */
export const REPORT_PAGE_DESCRIPTION_ID = 'report-a-bug-description';

/** From this many code points on, the description shows its count. */
export const COUNT_SHOWN_FROM = 3600;

/** The items a reporter ticks or unticks. */
export type TickItem =
	'page' | 'pageType' | 'errorStatus' | 'webVersion' | 'browser' | 'window' | 'display';

/** Which items go with the report. */
export type BugReportTicks = Readonly<Record<TickItem, boolean>>;

/** The ticks a fresh draft starts with: everything but the display settings. */
export const FRESH_TICKS: BugReportTicks = {
	page: true,
	pageType: true,
	errorStatus: true,
	webVersion: true,
	browser: true,
	window: true,
	display: false
};

/** Why a send was refused, without the reporter's text (the draft still holds it). */
export type SendProblem =
	BugReportRefusalKind | { readonly kind: 'limit' } | { readonly kind: 'couldNotSend' };

/** Where the draft's send stands. */
export type SendStatus =
	| { readonly status: 'editing' }
	| { readonly status: 'sending' }
	| { readonly status: 'refused'; readonly problem: SendProblem }
	| { readonly status: 'sent'; readonly reference: BugReference };

/** What an action's answer means for the draft. */
export type SendOutcome =
	| { readonly kind: 'sent'; readonly reference: BugReference }
	| { readonly kind: 'refused'; readonly problem: SendProblem }
	| { readonly kind: 'redirect'; readonly location: string };

/** Extra items an opener adds to the report (the error view adds its status). */
export interface BugReportExtras {
	/** The error page's HTTP status, when the error view opened the form; else `undefined`. */
	readonly errorStatus: number | undefined;
}

/** No extra items. */
export const NO_EXTRAS: BugReportExtras = { errorStatus: undefined };

/** The browser's display preferences. */
export interface DisplaySettings {
	readonly reducedMotion: boolean;
	readonly moreContrast: boolean;
	readonly forcedColors: boolean;
}

/** What the browser says when the form opens, before anything is ticked. */
export interface SnapshotSource {
	readonly pathname: string;
	readonly showingArchived: boolean;
	readonly userAgent: string;
	readonly width: number;
	readonly height: number;
	readonly display: DisplaySettings;
	readonly extras: BugReportExtras;
}

/** The values the dialog offers to send, read when it opens. */
export interface BugReportSnapshot {
	/** The page's path, with the archived words when it shows archived items; never a query. */
	readonly page: string;
	readonly pageType: PageType;
	/** The browser's text, cut to fit. */
	readonly browser: string;
	readonly width: number;
	readonly height: number;
	readonly display: DisplaySettings;
	readonly extras: BugReportExtras;
}

/** The snapshot of `source`: opened from the error view, its page type is "Error page". */
export function snapshotOf(source: SnapshotSource): BugReportSnapshot {
	return {
		page: source.showingArchived ? `${source.pathname}${ARCHIVED_SUFFIX}` : source.pathname,
		pageType: source.extras.errorStatus === undefined ? pageTypeOf(source.pathname) : 'Error page',
		browser: cutBrowser(source.userAgent),
		width: source.width,
		height: source.height,
		display: source.display,
		extras: source.extras
	};
}

/** The snapshot of the open page and this browser, now. */
export function browserSnapshot(extras: BugReportExtras): BugReportSnapshot {
	const matches = (query: string): boolean => window.matchMedia(query).matches;
	return snapshotOf({
		pathname: page.url.pathname,
		showingArchived: page.data.den?.includeDeleted === true,
		userAgent: navigator.userAgent,
		width: window.innerWidth,
		height: window.innerHeight,
		display: {
			reducedMotion: matches('(prefers-reduced-motion: reduce)'),
			moreContrast: matches('(prefers-contrast: more)'),
			forcedColors: matches('(forced-colors: active)')
		},
		extras
	});
}

/** One posted field. */
export interface PostedField {
	readonly name: BugReportFieldName;
	readonly value: string;
}

/** One tick box under "Sent with your report". */
export interface SentItem {
	readonly item: TickItem;
	/** The tick box's label: the item's name. */
	readonly label: string;
	/** Exactly what is sent, shown beneath the label. */
	readonly value: string;
	/** A note shown under the value. */
	readonly note: string | undefined;
	/** The fields posted while the box is ticked; none for the version, which the server stamps. */
	readonly fields: readonly PostedField[];
}

const PAGE_NOTE = 'This address can contain the ids of your Accounts, characters and commissions.';
const DISPLAY_NOTE = 'Helps with display bugs. These settings can say something about your needs.';

/** `on` or `off`. */
function onOff(on: boolean): string {
	return on ? 'on' : 'off';
}

/** What a matched or unmatched display preference posts. */
function matched(on: boolean): string {
	return on ? DISPLAY_MATCHED : DISPLAY_UNMATCHED;
}

/** The display settings as the reporter reads them. */
export function displayText(display: DisplaySettings): string {
	return [
		`reduce motion ${onOff(display.reducedMotion)}`,
		`more contrast ${onOff(display.moreContrast)}`,
		`forced colours ${onOff(display.forcedColors)}`
	].join(' · ');
}

function pageItem(value: string): SentItem {
	return {
		item: 'page',
		label: 'Page',
		value,
		note: PAGE_NOTE,
		fields: [{ name: BUG_REPORT_FIELDS.page, value }]
	};
}

function pageTypeItem(value: PageType): SentItem {
	return {
		item: 'pageType',
		label: 'Page type',
		value,
		note: undefined,
		fields: [{ name: BUG_REPORT_FIELDS.pageType, value }]
	};
}

/** The error view's status as an item: only a whole 400 to 599 is one, anything else is none. */
function errorStatusItems(status: number | undefined): readonly SentItem[] {
	if (status === undefined || !Number.isInteger(status) || !isRenderableErrorStatus(status)) {
		return [];
	}
	const value = String(status);
	return [
		{
			item: 'errorStatus',
			label: 'Error',
			value: `Status ${value} on this page`,
			note: undefined,
			fields: [{ name: BUG_REPORT_FIELDS.errorStatus, value }]
		}
	];
}

function versionItem(buildId: string): SentItem {
	return {
		item: 'webVersion',
		label: 'Zurfur version',
		value: buildId,
		note: undefined,
		fields: []
	};
}

function browserItem(value: string): SentItem {
	return {
		item: 'browser',
		label: 'Browser',
		value,
		note: undefined,
		fields: [{ name: BUG_REPORT_FIELDS.browser, value }]
	};
}

/** What the dialog offers to send, from its snapshot. */
export function snapshotItems(snapshot: BugReportSnapshot, buildId: string): readonly SentItem[] {
	const { display } = snapshot;
	return [
		pageItem(snapshot.page),
		pageTypeItem(snapshot.pageType),
		...errorStatusItems(snapshot.extras.errorStatus),
		versionItem(buildId),
		browserItem(snapshot.browser),
		{
			item: 'window',
			label: 'Window size',
			value: `${String(snapshot.width)} × ${String(snapshot.height)}`,
			note: undefined,
			fields: [
				{ name: BUG_REPORT_FIELDS.windowWidth, value: String(snapshot.width) },
				{ name: BUG_REPORT_FIELDS.windowHeight, value: String(snapshot.height) }
			]
		},
		{
			item: 'display',
			label: 'Display settings',
			value: displayText(display),
			note: DISPLAY_NOTE,
			fields: [
				{ name: BUG_REPORT_FIELDS.reducedMotion, value: matched(display.reducedMotion) },
				{ name: BUG_REPORT_FIELDS.moreContrast, value: matched(display.moreContrast) },
				{ name: BUG_REPORT_FIELDS.forcedColors, value: matched(display.forcedColors) }
			]
		}
	];
}

/**
 * What the report page offers to send, from its load: the page it came from
 * when known (else the form asks "Which page?"), its page type and the
 * browser when the load read them, and the version. Only script can read the
 * window or the display settings.
 */
export function offerItems(offer: BugReportOffer, buildId: string): readonly SentItem[] {
	return [
		...(offer.page.kind === 'known' ? [pageItem(offer.page.page)] : []),
		...(offer.pageType === undefined ? [] : [pageTypeItem(offer.pageType)]),
		versionItem(buildId),
		...(offer.browser === undefined ? [] : [browserItem(offer.browser)])
	];
}

/** A failure without the reporter's text. */
export function problemOf(failure: BugReportFailure): SendProblem {
	switch (failure.kind) {
		case 'descriptionEmpty':
		case 'descriptionTooLong':
		case 'pageAddress':
		case 'limit':
		case 'couldNotSend':
			return { kind: failure.kind };
		case 'hiddenCharacter':
			return { kind: 'hiddenCharacter', item: failure.item };
		case 'itemRefused':
			return { kind: 'itemRefused', item: failure.item };
	}
}

const COULD_NOT_SEND: SendOutcome = { kind: 'refused', problem: { kind: 'couldNotSend' } };

/**
 * What the report action's answer means: a success carrying a reference is
 * sent; a failure is refused by its kind; anything unreadable, and any error
 * (the gate's 503, a refused cross-site post), couldn't be sent; a redirect
 * goes where it points.
 */
export function outcomeOf(result: ActionResult): SendOutcome {
	switch (result.type) {
		case 'success': {
			const data: unknown = result.data;
			return isBugReportActionData(data) && data.kind === 'sent'
				? { kind: 'sent', reference: data.reference }
				: COULD_NOT_SEND;
		}
		case 'failure': {
			const data: unknown = result.data;
			return isBugReportActionData(data) && data.kind !== 'sent'
				? { kind: 'refused', problem: problemOf(data) }
				: COULD_NOT_SEND;
		}
		case 'error':
			return COULD_NOT_SEND;
		case 'redirect':
			return { kind: 'redirect', location: result.location };
	}
}

/** How a draft starts: empty, or from what the report page's action answered without script. */
export interface DraftStart {
	readonly description: string;
	readonly pageText: string;
	readonly status: SendStatus;
}

const EMPTY_START: DraftStart = { description: '', pageText: '', status: { status: 'editing' } };

/** The draft a report page starts with after a post without script answered `answer`. */
export function draftStartOf(answer: unknown): DraftStart {
	if (!isBugReportActionData(answer)) return EMPTY_START;
	return startFrom(answer);
}

function startFrom(answer: BugReportActionData): DraftStart {
	if (answer.kind === 'sent') {
		return { ...EMPTY_START, status: { status: 'sent', reference: answer.reference } };
	}
	return {
		description: answer.description,
		pageText: answer.page,
		status: { status: 'refused', problem: problemOf(answer) }
	};
}

/** A send in flight, so its answer can't land in a draft cleared meanwhile. */
export type SendTicket = number;

/** The reporter's text and ticks, kept until a send goes in, and where that send stands. */
export class BugReportDraft {
	/** "What happened?". */
	description = $state('');
	/** "Which page?", on the report page when it doesn't know the page. */
	pageText = $state('');
	ticks: BugReportTicks = $state.raw(FRESH_TICKS);
	status: SendStatus = $state.raw({ status: 'editing' });
	#generation = 0;

	constructor(start: DraftStart = EMPTY_START) {
		this.description = start.description;
		this.pageText = start.pageText;
		this.status = start.status;
	}

	/** Whether a send is in flight. */
	get sending(): boolean {
		return this.status.status === 'sending';
	}

	/** Tick or untick `item`. */
	setTick(item: TickItem, ticked: boolean): void {
		this.ticks = { ...this.ticks, [item]: ticked };
	}

	/** Start a send, unless one is in flight already. */
	beginSend(): SendTicket | undefined {
		if (this.sending) return undefined;
		this.status = { status: 'sending' };
		return this.#generation;
	}

	/** Take in the answer to the send `ticket` started: sent clears the text and ticks. */
	settle(ticket: SendTicket, outcome: SendOutcome): void {
		if (ticket !== this.#generation) return;
		switch (outcome.kind) {
			case 'sent':
				this.description = '';
				this.pageText = '';
				this.ticks = FRESH_TICKS;
				this.status = { status: 'sent', reference: outcome.reference };
				return;
			case 'refused':
				this.status = { status: 'refused', problem: outcome.problem };
				return;
			case 'redirect':
				this.status = { status: 'editing' };
				return;
		}
	}

	/** The reporter has seen the sent message: the next form starts fresh. */
	seen(): void {
		if (this.status.status === 'sent') this.status = { status: 'editing' };
	}

	/** Forget the text, the ticks and any send in flight. */
	clear(): void {
		this.description = '';
		this.pageText = '';
		this.ticks = FRESH_TICKS;
		this.status = { status: 'editing' };
		this.#generation += 1;
	}
}

/** The frame's bug report: the dialog, its draft, its snapshot and who opened it. */
export class BugReport {
	/** Whether the dialog is open. */
	isOpen = $state(false);
	/** What the dialog offers to send, re-read at each open. */
	snapshot: BugReportSnapshot | undefined = $state.raw();
	/** The control that opened the dialog, which gets focus back when it closes. */
	opener: HTMLElement | undefined = $state.raw();
	readonly draft = new BugReportDraft();
	#owner: Did;
	readonly #takeSnapshot: (extras: BugReportExtras) => BugReportSnapshot;

	constructor(
		owner: Did,
		takeSnapshot: (extras: BugReportExtras) => BugReportSnapshot = browserSnapshot
	) {
		this.#owner = owner;
		this.#takeSnapshot = takeSnapshot;
	}

	/**
	 * Open the dialog from `opener` (the clicked control, never whatever has
	 * focus: Safari doesn't focus a clicked button or link), reading the
	 * snapshot afresh and keeping the draft.
	 */
	open(opener: HTMLElement, extras: BugReportExtras = NO_EXTRAS): void {
		this.opener = opener;
		this.snapshot = this.#takeSnapshot(extras);
		this.isOpen = true;
	}

	/** Close the dialog; a sent message, once seen, gives way to a fresh form. */
	close(): void {
		this.isOpen = false;
		this.draft.seen();
	}

	/** Start over when the visitor is someone else. */
	ensureOwner(owner: Did): void {
		if (owner === this.#owner) return;
		this.#owner = owner;
		this.isOpen = false;
		this.snapshot = undefined;
		this.opener = undefined;
		this.draft.clear();
	}
}

/** The context key the frame shares its bug report under (exported so a spec can provide one). */
export const BUG_REPORT_KEY = Symbol('bug-report');

/** Share `report` with everything inside the frame. */
export function provideBugReport(report: BugReport): void {
	setContext(BUG_REPORT_KEY, report);
}

/** The frame's bug report, when the component sits inside the frame. */
export function bugReportFromContext(): BugReport | undefined {
	const report: unknown = getContext(BUG_REPORT_KEY);
	return report instanceof BugReport ? report : undefined;
}

/** The name of an item in a message. */
export function itemName(item: BugReportItem): string {
	switch (item) {
		case 'description':
			return 'What happened?';
		case 'page':
			return 'Page';
		case 'pageType':
			return 'Page type';
		case 'webVersion':
			return 'Zurfur version';
		case 'browser':
			return 'Browser';
		case 'window':
			return 'Window size';
		case 'display':
			return 'Display settings';
		case 'errorStatus':
			return 'Error';
	}
}

/** The form's line about a stack that doesn't keep reports, or `undefined` for a real one. */
export function testStackLine(stack: TestStack): string | undefined {
	switch (stack) {
		case 'devProfile':
			return 'This test stack forgets reports when it stops.';
		case 'mock':
			return "Mock mode doesn't keep reports.";
		case 'none':
			return undefined;
	}
}

/** A form field a refusal can be said on: "What happened?", or the report page's "Which page?". */
export type MessageField = 'description' | 'page';

/** Where a refusal is said: on its field, or in the form's status region. */
export type ProblemMessage =
	| { readonly place: 'field'; readonly field: MessageField; readonly text: string }
	| { readonly place: 'status'; readonly tone: 'info' | 'error'; readonly text: string };

const COULD_NOT_SEND_TEXT = "Couldn't send your report. Your text is still here; please try again.";

/** A calm status message. */
function info(text: string): ProblemMessage {
	return { place: 'status', tone: 'info', text };
}

/** A message on `field`. */
function onField(field: MessageField, text: string): ProblemMessage {
	return { place: 'field', field, text };
}

/** What the form says about `problem`; `askingPage` when the page is the reporter's own "Which page?" text. */
export function problemMessage(problem: SendProblem, askingPage: boolean): ProblemMessage {
	switch (problem.kind) {
		case 'descriptionEmpty':
			return onField('description', 'Tell us what happened.');
		case 'descriptionTooLong':
			return onField(
				'description',
				'Your text is longer than 4,000 characters. Please shorten it.'
			);
		case 'hiddenCharacter':
			return hiddenCharacterMessage(problem.item, askingPage);
		case 'pageAddress':
			return askingPage
				? onField('page', "Paste the page's address, or leave this empty.")
				: info(untickText('page'));
		case 'itemRefused':
			return info(
				problem.item === 'description'
					? "Your text couldn't be sent. Please change it and try again."
					: untickText(problem.item)
			);
		case 'limit':
			return info("You've sent several reports in the last hour. Please try again a little later.");
		case 'couldNotSend':
			return { place: 'status', tone: 'error', text: COULD_NOT_SEND_TEXT };
	}
}

/** `message`'s text when it is said on `field`, else `undefined`. */
export function fieldText(
	message: ProblemMessage | undefined,
	field: MessageField
): string | undefined {
	if (message === undefined) return undefined;
	switch (message.place) {
		case 'field':
			return message.field === field ? message.text : undefined;
		case 'status':
			return undefined;
	}
}

/** The per-item wording: the item couldn't be sent, untick it to send the rest. */
function untickText(item: BugReportItem): string {
	return `${itemName(item)} couldn't be sent. Untick ${itemName(item)} to send.`;
}

function hiddenCharacterMessage(
	item: 'description' | 'page' | 'browser',
	askingPage: boolean
): ProblemMessage {
	switch (item) {
		case 'description':
			return onField(
				'description',
				'Your text has a hidden formatting character. Please remove it.'
			);
		case 'page':
			return askingPage
				? onField('page', "Your page's address has a hidden character. Please remove it.")
				: info("Your page's address has a hidden character. Untick Page to send.");
		case 'browser':
			return info("Your browser's text has a hidden character. Untick Browser to send.");
	}
}
