import { Effect } from 'effect';
import { describe, expect, it, vi } from 'vitest';
import type { Problem } from '$lib/api/problem';
import { bugReference, firstPartyOrigin, type BugReference } from '$lib/types/brand';
import type { BugDescription } from '$lib/types/bug-report-rules';
import { ApiProblem, ContractViolation, NetworkFailure, NotAuthenticated } from './api/errors';
import { zurfurApiTest, type BugReportFiling, type ZurfurApiShape } from './api/zurfur-api';

vi.mock('$env/dynamic/private', () => ({ env: {} }));

const { fileBugReportOutcome, judgePostedPage, pageOfReferer } = await import('./bug-report');

const FIRST_PARTY = firstPartyOrigin('http://127.0.0.1:8080');
const REFERENCE = referenceOf('3f9a1c2e');

/** `raw` minted as a reference; the spec's own fixtures always mint. */
function referenceOf(raw: string): BugReference {
	const minted = bugReference(raw);
	if (minted === undefined) throw new Error(`not a reference: ${raw}`);
	return minted;
}

const FILING: BugReportFiling = {
	description: 'Broken.' as BugDescription,
	page: undefined,
	pageType: undefined,
	webVersion: undefined,
	browser: undefined,
	window: undefined,
	display: undefined,
	errorStatus: undefined
};

/** The outcome of filing {@link FILING} against a port whose call answers `answer`. */
function outcomeWhen(answer: ReturnType<ZurfurApiShape['fileBugReport']>) {
	const program = fileBugReportOutcome(FILING).pipe(
		Effect.provide(zurfurApiTest({ fileBugReport: () => answer }))
	);
	return Effect.runPromise(program);
}

function problemAt(status: number): Problem {
	return { type: 'urn:zurfur:error:x', code: 'x', title: 'x', detail: 'x', status };
}

describe('fileBugReportOutcome', () => {
	it('hands the filing to the port and answers sent with the reference (201)', async () => {
		const seen: BugReportFiling[] = [];
		const program = fileBugReportOutcome(FILING).pipe(
			Effect.provide(
				zurfurApiTest({
					fileBugReport: (filing) => {
						seen.push(filing);
						return Effect.succeed(REFERENCE);
					}
				})
			)
		);

		await expect(Effect.runPromise(program)).resolves.toStrictEqual({
			kind: 'sent',
			reference: REFERENCE
		});
		expect(seen).toStrictEqual([FILING]);
	});

	it('answers the limit for a 429', async () => {
		await expect(
			outcomeWhen(Effect.fail(new ApiProblem({ problem: problemAt(429) })))
		).resolves.toStrictEqual({ kind: 'limit' });
	});

	it('answers the limit for a 429 that carried no problem body', async () => {
		const violation = new ContractViolation({ path: '/bug-reports', status: 429, detail: 'x' });
		await expect(outcomeWhen(Effect.fail(violation))).resolves.toStrictEqual({ kind: 'limit' });
	});

	it.each([
		['the session problem', new NotAuthenticated({ problem: problemAt(401) })],
		['another 401 problem', new ApiProblem({ problem: problemAt(401) })],
		['a 401 with no problem body', new ContractViolation({ path: '/x', status: 401, detail: 'x' })]
	])('answers signed out for %s', async (_name, failure) => {
		await expect(outcomeWhen(Effect.fail(failure))).resolves.toStrictEqual({ kind: 'signedOut' });
	});

	it.each([
		['a 422', new ApiProblem({ problem: problemAt(422) })],
		['a 403', new ApiProblem({ problem: problemAt(403) })],
		['a 500', new ApiProblem({ problem: problemAt(500) })],
		['a 503', new ApiProblem({ problem: problemAt(503) })],
		['a 502 with no problem body', new ContractViolation({ path: '/x', status: 502, detail: 'x' })],
		['a malformed success', new ContractViolation({ path: '/x', status: 201, detail: 'x' })],
		['an unreachable API', new NetworkFailure({ reason: 'unreachable' })],
		['a timeout', new NetworkFailure({ reason: 'timeout' })]
	])('answers could not send for %s', async (_name, failure) => {
		await expect(outcomeWhen(Effect.fail(failure))).resolves.toStrictEqual({
			kind: 'couldNotSend'
		});
	});
});

describe('pageOfReferer', () => {
	it.each([
		['http://127.0.0.1:8080/den/x', '/den/x', 'My Den'],
		[
			'http://127.0.0.1:8080/den/x?includeDeleted=true&pageToken=abc',
			'/den/x (showing archived)',
			'My Den'
		],
		['http://127.0.0.1:8080/den?includeDeleted=true&includeDeleted=true', '/den', 'My Den'],
		['http://127.0.0.1:8080/accounts#top', '/accounts', 'Accounts'],
		['http://127.0.0.1:8080/', '/', 'Other']
	])('reads %j as %j (%s)', (referer, page, pageType) => {
		expect(pageOfReferer(referer, FIRST_PARTY)).toStrictEqual({ page, pageType });
	});

	it.each([
		'http://127.0.0.1:9999/den',
		'http://localhost:8080/den',
		'http://127.0.0.1:8080.evil.example/',
		'http://127.0.0.1:80800/den',
		'https://127.0.0.1:8080/den',
		'/den',
		'',
		'not a url'
	])('reads nothing from %j', (referer) => {
		expect(pageOfReferer(referer, FIRST_PARTY)).toBeUndefined();
	});

	it.each([
		'http://127.0.0.1:8080/report-a-bug',
		'http://127.0.0.1:8080/report-a-bug/',
		'http://127.0.0.1:8080/report-a-bug?x=1#top'
	])(
		'reads nothing from the report page itself (%j): a post without script came back from there',
		(referer) => {
			expect(pageOfReferer(referer, FIRST_PARTY)).toBeUndefined();
		}
	);

	it('still reads a page below the report page’s address as an ordinary page', () => {
		expect(pageOfReferer('http://127.0.0.1:8080/report-a-bug/x', FIRST_PARTY)).toStrictEqual({
			page: '/report-a-bug/x',
			pageType: 'Other'
		});
	});

	it('reads nothing with no Referer, or with no first party configured', () => {
		expect(pageOfReferer(undefined, FIRST_PARTY)).toBeUndefined();
		expect(pageOfReferer('http://127.0.0.1:8080/den', undefined)).toBeUndefined();
	});
});

describe('the first-party yardstick is an exact origin, never a prefix', () => {
	const portless = firstPartyOrigin('https://zurfur.app');

	it.each([
		'https://zurfur.app.evil.example/den',
		'https://zurfur.app@evil.example/den',
		'https://zurfur.application/den'
	])('refuses %j for https://zurfur.app', (address) => {
		expect(pageOfReferer(address, portless)).toBeUndefined();
		expect(judgePostedPage(address, portless)).toStrictEqual({ kind: 'notAnAddress' });
	});

	it('refuses an address whose text starts with the first party but whose host is another', () => {
		const address = 'http://127.0.0.1:8080@evil.example/den';
		expect(pageOfReferer(address, FIRST_PARTY)).toBeUndefined();
		expect(judgePostedPage(address, FIRST_PARTY)).toStrictEqual({ kind: 'notAnAddress' });
	});

	it('accepts the first party itself', () => {
		expect(pageOfReferer('https://zurfur.app/den', portless)).toStrictEqual({
			page: '/den',
			pageType: 'My Den'
		});
	});
});

describe('judgePostedPage', () => {
	it.each([
		['/den/x', '/den/x'],
		['  /den/x  ', '/den/x'],
		['/den/x?includeDeleted=true#frag', '/den/x (showing archived)'],
		['/den/x (showing archived)', '/den/x (showing archived)'],
		['http://127.0.0.1:8080/den/x?pageToken=abc', '/den/x'],
		['HTTP://127.0.0.1:8080/den/x', '/den/x']
	])('accepts %j as %j', (posted, page) => {
		expect(judgePostedPage(posted, FIRST_PARTY)).toStrictEqual({ kind: 'accepted', page });
	});

	it.each(['', '   '])('does not send %j', (posted) => {
		expect(judgePostedPage(posted, FIRST_PARTY)).toStrictEqual({ kind: 'notSent' });
	});

	it.each([
		'http://localhost:8080/den',
		'http://127.0.0.1:8080.evil.example/den',
		'//evil.example/den',
		'/\\evil.example/den',
		'den/x',
		'javascript:alert(1)',
		`/${'a'.repeat(2048)}`
	])('refuses %j as not an address', (posted) => {
		expect(judgePostedPage(posted, FIRST_PARTY)).toStrictEqual({ kind: 'notAnAddress' });
	});

	it.each([0x202e, 0x9b, 0x0a])('refuses U+%s before reading it as an address', (codePoint) => {
		const posted = `/den/${String.fromCodePoint(codePoint)}x`;
		expect(judgePostedPage(posted, FIRST_PARTY)).toStrictEqual({ kind: 'hiddenCharacter' });
	});

	it('refuses every page when no first party is configured', () => {
		expect(judgePostedPage('/den/x', undefined)).toStrictEqual({ kind: 'notAnAddress' });
	});
});
