import { describe, expect, it } from 'vitest';
import type { BugReportOffer } from '$lib/api/bug-report';
import { bugReference, did, type BugReference } from '$lib/types/brand';
import type { BugBrowser, BugPage } from '$lib/types/bug-report-rules';
import {
	BugReport,
	BugReportDraft,
	displayText,
	draftStartOf,
	FRESH_TICKS,
	offerItems,
	outcomeOf,
	itemName,
	snapshotItems,
	snapshotOf,
	testStackLine,
	type BugReportSnapshot,
	type SnapshotSource
} from './bug-report.svelte';

/** A reference the brand accepts. */
function reference(raw: string): BugReference {
	const minted = bugReference(raw);
	if (minted === undefined) throw new Error(`not a reference: ${raw}`);
	return minted;
}

const REFERENCE = reference('BR-1');

const source: SnapshotSource = {
	pathname: '/den/x',
	showingArchived: false,
	userAgent: 'Mozilla/5.0',
	width: 390,
	height: 844,
	display: { reducedMotion: true, moreContrast: false, forcedColors: false },
	extras: { errorStatus: undefined }
};

/** A snapshot that always says `source`, for a frame under test. */
function fixedSnapshot(): BugReportSnapshot {
	return snapshotOf(source);
}

describe('snapshotOf', () => {
	it('is the path, its page type, the browser cut to fit and the window', () => {
		const snapshot = snapshotOf({ ...source, userAgent: 'x'.repeat(600) });
		expect(snapshot.page).toBe('/den/x');
		expect(snapshot.pageType).toBe('My Den');
		expect(new TextEncoder().encode(snapshot.browser).length).toBeLessThanOrEqual(512);
		expect(snapshot.browser.endsWith('…')).toBe(true);
	});

	it('adds the archived words when the page shows archived items', () => {
		expect(snapshotOf({ ...source, showingArchived: true }).page).toBe('/den/x (showing archived)');
	});
});

describe('the items offered', () => {
	it('says the display settings in words and posts each as matched or not', () => {
		expect(displayText(source.display)).toBe(
			'reduce motion on · more contrast off · forced colours off'
		);
		const display = snapshotItems(fixedSnapshot(), 'web-1').find((item) => item.item === 'display');
		expect(display?.fields).toEqual([
			{ name: 'reducedMotion', value: 'true' },
			{ name: 'moreContrast', value: 'false' },
			{ name: 'forcedColors', value: 'false' }
		]);
	});

	it('posts no value for the version: the server stamps its own', () => {
		const version = snapshotItems(fixedSnapshot(), 'web-1').find(
			(item) => item.item === 'webVersion'
		);
		expect(version?.value).toBe('web-1');
		expect(version?.fields).toEqual([]);
	});

	it('offers on the report page only what its load read, never the window or display', () => {
		const offer: BugReportOffer = {
			page: { kind: 'ask' },
			pageType: undefined,
			browser: 'Mozilla/5.0' as BugBrowser
		};
		expect(offerItems(offer, 'web-1').map((item) => item.item)).toEqual(['webVersion', 'browser']);
		const known: BugReportOffer = {
			page: { kind: 'known', page: '/accounts' as BugPage },
			pageType: 'Accounts',
			browser: undefined
		};
		expect(offerItems(known, 'web-1').map((item) => item.item)).toEqual([
			'page',
			'pageType',
			'webVersion'
		]);
	});

	it('starts with everything ticked but the display settings', () => {
		expect(FRESH_TICKS).toEqual({
			page: true,
			pageType: true,
			errorStatus: true,
			webVersion: true,
			browser: true,
			window: true,
			display: false
		});
	});
});

describe('outcomeOf', () => {
	it('reads a success carrying a reference as sent', () => {
		expect(
			outcomeOf({ type: 'success', status: 200, data: { kind: 'sent', reference: 'BR-1' } })
		).toEqual({ kind: 'sent', reference: 'BR-1' });
	});

	it('reads a success without a reference as could not send', () => {
		expect(outcomeOf({ type: 'success', status: 200 })).toEqual({
			kind: 'refused',
			problem: { kind: 'couldNotSend' }
		});
	});

	it('reads a failure by its kind, without the echoed text', () => {
		const data = { kind: 'itemRefused', item: 'browser', description: 'x', page: '' };
		expect(outcomeOf({ type: 'failure', status: 400, data })).toEqual({
			kind: 'refused',
			problem: { kind: 'itemRefused', item: 'browser' }
		});
	});

	it('reads an unreadable failure, and any error, as could not send', () => {
		const couldNotSend = { kind: 'refused', problem: { kind: 'couldNotSend' } };
		expect(outcomeOf({ type: 'failure', status: 400, data: { kind: 'odd' } })).toEqual(
			couldNotSend
		);
		expect(outcomeOf({ type: 'error', status: 503, error: new Error('down') })).toEqual(
			couldNotSend
		);
	});

	it('follows a redirect', () => {
		expect(outcomeOf({ type: 'redirect', status: 303, location: '/login' })).toEqual({
			kind: 'redirect',
			location: '/login'
		});
	});
});

describe('BugReportDraft', () => {
	it('sends once at a time', () => {
		const draft = new BugReportDraft();
		expect(draft.beginSend()).toBeDefined();
		expect(draft.beginSend()).toBeUndefined();
	});

	it('clears the text and ticks after a send, keeping them after a refusal', () => {
		const draft = new BugReportDraft();
		draft.description = 'It broke';
		draft.setTick('page', false);
		const refusedTicket = draft.beginSend() ?? -1;
		draft.settle(refusedTicket, { kind: 'refused', problem: { kind: 'limit' } });
		expect(draft.description).toBe('It broke');
		expect(draft.ticks.page).toBe(false);

		const sentTicket = draft.beginSend() ?? -1;
		draft.settle(sentTicket, { kind: 'sent', reference: REFERENCE });
		expect(draft.description).toBe('');
		expect(draft.ticks).toEqual(FRESH_TICKS);
		expect(draft.status).toEqual({ status: 'sent', reference: REFERENCE });
	});

	it('ignores the answer to a send started before it was cleared', () => {
		const draft = new BugReportDraft();
		const ticket = draft.beginSend() ?? -1;
		draft.clear();
		draft.description = 'New';
		draft.settle(ticket, { kind: 'sent', reference: REFERENCE });
		expect(draft.description).toBe('New');
		expect(draft.status).toEqual({ status: 'editing' });
	});

	it('starts the report page from what a post without script answered', () => {
		expect(draftStartOf({ kind: 'limit', description: 'Mine', page: '/den/y' })).toEqual({
			description: 'Mine',
			pageText: '/den/y',
			status: { status: 'refused', problem: { kind: 'limit' } }
		});
		expect(draftStartOf({ kind: 'sent', reference: 'BR-1' }).status).toEqual({
			status: 'sent',
			reference: 'BR-1'
		});
		expect(draftStartOf(undefined).status).toEqual({ status: 'editing' });
	});
});

describe('BugReport', () => {
	it('keeps the draft over close and reopen, and shows a sent message until it is seen', () => {
		const report = new BugReport(did('did:plc:alice'), fixedSnapshot);
		const opener = document.createElement('button');
		report.open(opener);
		report.draft.description = 'Kept';
		report.close();
		expect(report.draft.description).toBe('Kept');

		const ticket = report.draft.beginSend() ?? -1;
		report.draft.settle(ticket, { kind: 'sent', reference: REFERENCE });
		report.open(opener);
		expect(report.draft.status.status).toBe('sent');
		expect(report.opener).toBe(opener);
		report.close();
		expect(report.draft.status.status).toBe('editing');
	});

	it('starts over when the signed-in visitor changes', () => {
		const report = new BugReport(did('did:plc:alice'), fixedSnapshot);
		report.open(document.createElement('button'));
		report.draft.description = "Alice's";
		report.draft.setTick('browser', false);

		report.ensureOwner(did('did:plc:alice'));
		expect(report.draft.description).toBe("Alice's");

		report.ensureOwner(did('did:plc:bob'));
		expect(report.isOpen).toBe(false);
		expect(report.draft.description).toBe('');
		expect(report.draft.ticks).toEqual(FRESH_TICKS);
	});
});

describe('the error view’s status item', () => {
	const fromErrorView = (status: number): BugReportSnapshot =>
		snapshotOf({ ...source, extras: { errorStatus: status } });

	it('is "Error", ticked from the start, reading and posting the status', () => {
		const item = snapshotItems(fromErrorView(500), 'web-1').find(
			(offered) => offered.item === 'errorStatus'
		);
		expect(item).toStrictEqual({
			item: 'errorStatus',
			label: 'Error',
			value: 'Status 500 on this page',
			note: undefined,
			fields: [{ name: 'errorStatus', value: '500' }]
		});
		expect(FRESH_TICKS.errorStatus).toBe(true);
	});

	it('makes the page type "Error page"', () => {
		expect(fromErrorView(404).pageType).toBe('Error page');
		expect(snapshotOf(source).pageType).toBe('My Den');
	});

	it.each([400, 599])('offers status %i, the edges of 400 to 599', (status) => {
		const items = snapshotItems(fromErrorView(status), 'web-1');
		expect(items.some((offered) => offered.item === 'errorStatus')).toBe(true);
	});

	it.each([399, 600, 200, 302, 500.5, Number.NaN])('offers no item for status %d', (status) => {
		const items = snapshotItems(fromErrorView(status), 'web-1');
		expect(items.some((offered) => offered.item === 'errorStatus')).toBe(false);
	});

	it('is absent when no error view opened the form', () => {
		const items = snapshotItems(fixedSnapshot(), 'web-1');
		expect(items.some((offered) => offered.item === 'errorStatus')).toBe(false);
	});

	it('is named as its box is in a refusal', () => {
		expect(itemName('errorStatus')).toBe('Error');
	});
});

describe('testStackLine', () => {
	it('says the dev profile’s test stack forgets reports, mock mode keeps none, a real stack nothing', () => {
		expect(testStackLine('devProfile')).toBe('This test stack forgets reports when it stops.');
		expect(testStackLine('mock')).toBe("Mock mode doesn't keep reports.");
		expect(testStackLine('none')).toBeUndefined();
	});
});
