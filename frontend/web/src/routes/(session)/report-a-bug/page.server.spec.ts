import { isActionFailure } from '@sveltejs/kit';
import { Effect } from 'effect';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	BUG_REPORT_FIELDS as FIELD,
	DISPLAY_MATCHED,
	DISPLAY_UNMATCHED,
	TICKED,
	type BugReportOffer
} from '$lib/api/bug-report';
import type { Problem } from '$lib/api/problem';
import type { Trail } from '$lib/api/trail';
import { expectRedirect } from '$lib/testing/redirect';
import { bugReference, type BugReference } from '$lib/types/brand';
import { BROWSER_MAX_BYTES, utf8ByteLength } from '$lib/types/bug-report-rules';
import type { BugReportFiling, ZurfurApi, ZurfurApiShape } from '$lib/server/api/zurfur-api';
import { ApiProblem, NetworkFailure, NotAuthenticated } from '$lib/server/api/errors';

/** What the specs control: the run-time env, and the port's one bug-report call. */
interface Harness {
	readonly env: Record<string, string | undefined>;
	fileBugReport: ZurfurApiShape['fileBugReport'] | undefined;
}

const harness = vi.hoisted((): Harness => ({
	env: { ZURFUR_PUBLIC_URL: 'http://127.0.0.1:8080' },
	fileBugReport: undefined
}));

// The first party is http://127.0.0.1:8080, read once when the modules load.
vi.mock('$env/dynamic/private', () => ({ env: harness.env }));

// The seam runs every program against the in-memory port, with this spec's bug-report call.
vi.mock('$lib/server/runtime', async () => {
	const { Effect: RuntimeEffect } = await import('effect');
	const { zurfurApiTest } = await import('$lib/server/api/zurfur-api');
	return {
		runApi: <A, E>(_fetch: unknown, program: Effect.Effect<A, E, ZurfurApi>) => {
			const overrides =
				harness.fileBugReport === undefined ? {} : { fileBugReport: harness.fileBugReport };
			return RuntimeEffect.runPromise(
				program.pipe(RuntimeEffect.provide(zurfurApiTest(overrides)))
			);
		}
	};
});

const { actions, load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<(typeof actions)['default']>[0];

const FIRST_PARTY = 'http://127.0.0.1:8080';
const REFERENCE = referenceOf('3f9a1c2e');

/** `raw` minted as a reference; the spec's own fixtures always mint. */
function referenceOf(raw: string): BugReference {
	const minted = bugReference(raw);
	if (minted === undefined) throw new Error(`not a reference: ${raw}`);
	return minted;
}

const EMOJI = '😀';
const RLO = String.fromCodePoint(0x202e);
const CSI = String.fromCodePoint(0x9b);
const LINE_SEPARATOR = String.fromCodePoint(0x2028);
const PARAGRAPH_SEPARATOR = String.fromCodePoint(0x2029);

/** Every bug-report call the port received, in order. */
let filed: BugReportFiling[] = [];

/** The port answers `answer` to every filing, recording each. */
function portAnswers(
	answer: Effect.Effect<BugReference, ApiProblem | NotAuthenticated | NetworkFailure>
): void {
	harness.fileBugReport = (filing) => {
		filed.push(filing);
		return answer;
	};
}

beforeEach(() => {
	filed = [];
	harness.env.ZURFUR_BUILD_ID = 'build-2026.10.17';
	portAnswers(Effect.succeed(REFERENCE));
});

afterEach(() => {
	harness.fileBugReport = undefined;
});

/** One posted field, name then value. */
type Field = readonly [string, string];

/** Post `fields` as an enhanced form does (multipart). */
async function post(fields: readonly Field[]): Promise<unknown> {
	const body = new FormData();
	for (const [name, value] of fields) body.append(name, value);
	const request = new Request(`${FIRST_PARTY}/report-a-bug`, { method: 'POST', body });
	return actions.default({ request, fetch } as unknown as ActionEvent);
}

/** Post `fields` as a form without script does (urlencoded). */
async function postWithoutScript(fields: readonly Field[]): Promise<unknown> {
	const body = new URLSearchParams();
	for (const [name, value] of fields) body.append(name, value);
	const request = new Request(`${FIRST_PARTY}/report-a-bug`, { method: 'POST', body });
	return actions.default({ request, fetch } as unknown as ActionEvent);
}

/** The action's answer as SvelteKit would send it: a status and the data. */
interface Answer {
	readonly status: number;
	readonly data: unknown;
}

function answerOf(result: unknown): Answer {
	if (isActionFailure(result)) return { status: result.status, data: result.data };
	return { status: 200, data: result };
}

/** The one filing the port received. */
function onlyFiling(): BugReportFiling {
	expect(filed).toHaveLength(1);
	const [filing] = filed;
	if (filing === undefined) throw new Error('nothing was filed');
	return filing;
}

/** A filing of `description` with nothing else sent. */
function bare(description: string): BugReportFiling {
	return {
		description,
		page: undefined,
		pageType: undefined,
		webVersion: undefined,
		browser: undefined,
		window: undefined,
		display: undefined,
		errorStatus: undefined
	} as BugReportFiling;
}

/** Every item ticked, each with a value. */
const EVERY_ITEM: readonly Field[] = [
	[FIELD.description, 'The tree forgot my folder.'],
	[FIELD.sendPage, TICKED],
	[FIELD.page, '/den/x'],
	[FIELD.sendPageType, TICKED],
	[FIELD.pageType, 'My Den'],
	[FIELD.sendWebVersion, TICKED],
	[FIELD.sendBrowser, TICKED],
	[FIELD.browser, 'Mozilla/5.0 (X11; Linux x86_64)'],
	[FIELD.sendWindow, TICKED],
	[FIELD.windowWidth, '1280'],
	[FIELD.windowHeight, '720'],
	[FIELD.sendDisplay, TICKED],
	[FIELD.reducedMotion, DISPLAY_MATCHED],
	[FIELD.moreContrast, DISPLAY_UNMATCHED],
	[FIELD.forcedColors, DISPLAY_UNMATCHED],
	[FIELD.sendErrorStatus, TICKED],
	[FIELD.errorStatus, '404']
];

/** What {@link EVERY_ITEM} files. */
const EVERY_ITEM_FILED = {
	description: 'The tree forgot my folder.',
	page: '/den/x',
	pageType: 'My Den',
	webVersion: 'build-2026.10.17',
	browser: 'Mozilla/5.0 (X11; Linux x86_64)',
	window: { width: 1280, height: 720 },
	display: { reducedMotion: true, moreContrast: false, forcedColors: false },
	errorStatus: 404
};

describe('the report action: what it forwards', () => {
	it('forwards_only_known_fields', async () => {
		const forged: readonly Field[] = [
			['reporter', 'did:plc:mallory'],
			['did', 'did:plc:mallory'],
			['cookie', 'zurfur.sid=stolen']
		];

		const answer = answerOf(await post([...EVERY_ITEM, ...forged]));

		expect(answer).toStrictEqual({ status: 200, data: { kind: 'sent', reference: REFERENCE } });
		expect(onlyFiling()).toStrictEqual(EVERY_ITEM_FILED);
	});

	it.each([
		['page', FIELD.sendPage],
		['pageType', FIELD.sendPageType],
		['webVersion', FIELD.sendWebVersion],
		['browser', FIELD.sendBrowser],
		['window', FIELD.sendWindow],
		['display', FIELD.sendDisplay],
		['errorStatus', FIELD.sendErrorStatus]
	] as const)(
		'an_unticked_item_is_not_forwarded_even_when_its_value_is_posted (%s)',
		async (item, box) => {
			await post(EVERY_ITEM.filter(([name]) => name !== box));

			expect(onlyFiling()).toStrictEqual({ ...EVERY_ITEM_FILED, [item]: undefined });
		}
	);

	it('ignores an unticked item whose value would be refused', async () => {
		const answer = answerOf(
			await post([
				[FIELD.description, 'Broken.'],
				[FIELD.page, 'https://evil.example/'],
				[FIELD.browser, RLO],
				[FIELD.windowWidth, '-1'],
				[FIELD.errorStatus, '200']
			])
		);

		expect(answer.status).toBe(200);
		expect(onlyFiling()).toStrictEqual(bare('Broken.'));
	});

	it.each([
		['page', [FIELD.sendPage, FIELD.page]],
		['pageType', [FIELD.sendPageType, FIELD.pageType]],
		['browser', [FIELD.sendBrowser, FIELD.browser]]
	] as const)('never forwards a ticked but empty %s', async (_item, [box, value]) => {
		await post([
			[FIELD.description, 'Broken.'],
			[box, TICKED],
			[value, '   ']
		]);

		expect(onlyFiling()).toStrictEqual(bare('Broken.'));
	});

	it('trims every optional text', async () => {
		await post([
			[FIELD.description, '  Broken.  '],
			[FIELD.sendPage, TICKED],
			[FIELD.page, '  /accounts  '],
			[FIELD.sendBrowser, TICKED],
			[FIELD.browser, '  Firefox  ']
		]);

		expect(onlyFiling()).toStrictEqual({
			...bare('Broken.'),
			page: '/accounts',
			browser: 'Firefox'
		});
	});
});

describe('the report action: the page', () => {
	it.each([
		['/den/x?includeDeleted=true&pageToken=abc#x', '/den/x (showing archived)'],
		['/den/x (showing archived)', '/den/x (showing archived)'],
		['/den/x?pageToken=abc#x (showing archived)', '/den/x (showing archived)'],
		['/den/x?pageToken=abc#x', '/den/x'],
		[`${FIRST_PARTY}/den/x?pageToken=abc#x`, '/den/x'],
		[`${FIRST_PARTY}/den/x?includeDeleted=true`, '/den/x (showing archived)']
	])('forwards %j as exactly %j', async (posted, forwarded) => {
		await post([
			[FIELD.description, 'Broken.'],
			[FIELD.sendPage, TICKED],
			[FIELD.page, posted]
		]);

		expect(onlyFiling().page).toBe(forwarded);
	});

	it.each([
		'http://127.0.0.1:9999/den',
		'http://localhost:8080/den',
		'http://127.0.0.1:8080.evil.example/den',
		'http://127.0.0.1:8080@evil.example/den',
		'//evil.example/den',
		'/\\evil.example/den',
		'http://127.0.0.1:8080//evil.example/x',
		'http://127.0.0.1:8080/\\evil.example/a',
		'den/x',
		'not an address'
	])('refuses %j as "Which page?", keeping the text', async (posted) => {
		const answer = answerOf(
			await post([
				[FIELD.description, 'Broken.'],
				[FIELD.sendPage, TICKED],
				[FIELD.page, posted]
			])
		);

		expect(answer).toStrictEqual({
			status: 400,
			data: { kind: 'pageAddress', description: 'Broken.', page: posted }
		});
		expect(filed).toHaveLength(0);
	});
});

describe('the report action: the version', () => {
	it('stamps the server build when "Zurfur version" is ticked, never the posted value', async () => {
		await post([
			[FIELD.description, 'Broken.'],
			[FIELD.sendWebVersion, TICKED],
			['webVersion', 'forged']
		]);

		expect(onlyFiling().webVersion).toBe('build-2026.10.17');
	});

	it('stamps "unknown" when ZURFUR_BUILD_ID is unset', async () => {
		delete harness.env.ZURFUR_BUILD_ID;

		await post([
			[FIELD.description, 'Broken.'],
			[FIELD.sendWebVersion, TICKED],
			['webVersion', 'forged']
		]);

		expect(onlyFiling().webVersion).toBe('unknown');
	});
});

describe('the report action: the description and hidden characters', () => {
	it('passes 4,000 emoji and refuses 4,001, keeping the text', async () => {
		const atLimit = answerOf(await post([[FIELD.description, EMOJI.repeat(4000)]]));
		const overLimit = answerOf(await post([[FIELD.description, EMOJI.repeat(4001)]]));

		expect(atLimit.status).toBe(200);
		expect(overLimit).toStrictEqual({
			status: 400,
			data: { kind: 'descriptionTooLong', description: EMOJI.repeat(4001), page: '' }
		});
		expect(onlyFiling().description).toBe(EMOJI.repeat(4000));
	});

	it('passes 4,000 code points with CR LF line breaks from a post without script, forwarding LF', async () => {
		const lines = Array.from({ length: 1000 }, () => 'abc');
		const posted = `${lines.join('\r\n')}d`;
		expect(posted.replaceAll('\r\n', '\n')).toHaveLength(4000);

		const answer = answerOf(await postWithoutScript([[FIELD.description, posted]]));

		expect(answer.status).toBe(200);
		expect(onlyFiling().description).toBe(`${lines.join('\n')}d`);
	});

	it('refuses an empty description', async () => {
		const answer = answerOf(await post([[FIELD.description, ' \r\n ']]));

		expect(answer).toStrictEqual({
			status: 400,
			data: { kind: 'descriptionEmpty', description: ' \r\n ', page: '' }
		});
		expect(filed).toHaveLength(0);
	});

	it('refuses a post with no description at all', async () => {
		const answer = answerOf(await post([]));

		expect(answer).toStrictEqual({
			status: 400,
			data: { kind: 'descriptionEmpty', description: '', page: '' }
		});
	});

	it.each([
		['U+202E', RLO],
		['U+009B', CSI]
	])('refuses %s in the description', async (_name, character) => {
		const description = `Broken ${character} here.`;

		const answer = answerOf(await post([[FIELD.description, description]]));

		expect(answer).toStrictEqual({
			status: 400,
			data: { kind: 'hiddenCharacter', item: 'description', description, page: '' }
		});
		expect(filed).toHaveLength(0);
	});

	it.each([
		['U+202E', RLO],
		['U+009B', CSI],
		['U+2028', LINE_SEPARATOR],
		['U+2029', PARAGRAPH_SEPARATOR]
	])('refuses %s in "Which page?"', async (_name, character) => {
		const page = `/den/${character}x`;

		const answer = answerOf(
			await post([
				[FIELD.description, 'Broken.'],
				[FIELD.sendPage, TICKED],
				[FIELD.page, page]
			])
		);

		expect(answer).toStrictEqual({
			status: 400,
			data: { kind: 'hiddenCharacter', item: 'page', description: 'Broken.', page }
		});
		expect(filed).toHaveLength(0);
	});

	it.each([
		['U+202E', RLO],
		['U+009B', CSI],
		['U+2028', LINE_SEPARATOR],
		['U+2029', PARAGRAPH_SEPARATOR]
	])('refuses %s in the browser text', async (_name, character) => {
		const answer = answerOf(
			await post([
				[FIELD.description, 'Broken.'],
				[FIELD.sendBrowser, TICKED],
				[FIELD.browser, `Mozilla${character}5`]
			])
		);

		expect(answer).toStrictEqual({
			status: 400,
			data: { kind: 'hiddenCharacter', item: 'browser', description: 'Broken.', page: '' }
		});
		expect(filed).toHaveLength(0);
	});

	it('cuts a long browser text instead of refusing it', async () => {
		await post([
			[FIELD.description, 'Broken.'],
			[FIELD.sendBrowser, TICKED],
			[FIELD.browser, 'M'.repeat(2000)]
		]);

		const browser = onlyFiling().browser ?? '';
		expect(utf8ByteLength(browser)).toBe(BROWSER_MAX_BYTES);
		expect(browser.endsWith('…')).toBe(true);
	});

	it.each([
		[
			'pageType',
			[
				[FIELD.sendPageType, TICKED],
				[FIELD.pageType, 'Admin panel']
			]
		],
		[
			'window',
			[
				[FIELD.sendWindow, TICKED],
				[FIELD.windowWidth, '0'],
				[FIELD.windowHeight, '720']
			]
		],
		[
			'display',
			[
				[FIELD.sendDisplay, TICKED],
				[FIELD.reducedMotion, 'on']
			]
		],
		[
			'errorStatus',
			[
				[FIELD.sendErrorStatus, TICKED],
				[FIELD.errorStatus, '302']
			]
		]
	] as const)('refuses a %s outside its rule, naming it', async (item, fields) => {
		const answer = answerOf(await post([[FIELD.description, 'Broken.'], ...fields]));

		expect(answer).toStrictEqual({
			status: 400,
			data: { kind: 'itemRefused', item, description: 'Broken.', page: '' }
		});
		expect(filed).toHaveLength(0);
	});
});

/** A problem the API answers with `status`. */
function problemAt(status: number): Problem {
	return { type: 'urn:zurfur:error:x', code: 'x', title: 'x', detail: 'x', status };
}

describe('the report action: the API answer', () => {
	it('answers the limit (429) with the text kept', async () => {
		portAnswers(Effect.fail(new ApiProblem({ problem: problemAt(429) })));

		const answer = answerOf(
			await post([
				[FIELD.description, 'Broken.'],
				[FIELD.sendPage, TICKED],
				[FIELD.page, '/accounts']
			])
		);

		expect(answer).toStrictEqual({
			status: 429,
			data: { kind: 'limit', description: 'Broken.', page: '/accounts' }
		});
	});

	it.each([
		['a 422 backstop', Effect.fail(new ApiProblem({ problem: problemAt(422) }))],
		['a 403', Effect.fail(new ApiProblem({ problem: problemAt(403) }))],
		['a 500', Effect.fail(new ApiProblem({ problem: problemAt(500) }))],
		['an unreachable API', Effect.fail(new NetworkFailure({ reason: 'unreachable' }))],
		['a timeout', Effect.fail(new NetworkFailure({ reason: 'timeout' }))]
	])('answers %s as could not send (502), with the text kept', async (_name, answer) => {
		portAnswers(answer);

		const result = answerOf(await post([[FIELD.description, 'Broken.']]));

		expect(result).toStrictEqual({
			status: 502,
			data: { kind: 'couldNotSend', description: 'Broken.', page: '' }
		});
	});

	it('sends an ended session (401) to sign in', async () => {
		portAnswers(Effect.fail(new NotAuthenticated({ problem: problemAt(401) })));

		const redirect = await expectRedirect(() => post([[FIELD.description, 'Broken.']]));

		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/login');
	});

	it('answers data the dialog accepts', async () => {
		const sent = answerOf(await post([[FIELD.description, 'Broken.']]));
		const refused = answerOf(await post([[FIELD.description, '']]));

		const { isBugReportActionData } = await import('$lib/api/bug-report');
		expect(isBugReportActionData(sent.data)).toBe(true);
		expect(isBugReportActionData(refused.data)).toBe(true);
	});
});

/** What the load answers, past the generated type's noise. */
interface ReportLoadData {
	readonly offer: BugReportOffer;
	readonly trail: Trail;
}

/** The load for a request carrying `headers`. */
async function loadWith(headers: Record<string, string>): Promise<ReportLoadData> {
	const request = new Request(`${FIRST_PARTY}/report-a-bug`, { headers });
	return (await load({ request } as unknown as LoadEvent)) as ReportLoadData;
}

describe('the report page load', () => {
	it('takes the page from a Referer on the first party, without its query, with the archived words', async () => {
		const data = await loadWith({
			referer: `${FIRST_PARTY}/den/x?includeDeleted=true&pageToken=abc`
		});

		expect(data.offer.page).toStrictEqual({ kind: 'known', page: '/den/x (showing archived)' });
		expect(data.offer.pageType).toBe('My Den');
	});

	it('takes the page without the archived words when the Referer did not ask for them', async () => {
		const data = await loadWith({ referer: `${FIRST_PARTY}/accounts/abc?x=1#top` });

		expect(data.offer.page).toStrictEqual({ kind: 'known', page: '/accounts/abc' });
		expect(data.offer.pageType).toBe('An Account');
	});

	it.each([
		'http://127.0.0.1:9999/den',
		'http://localhost:8080/den',
		'http://127.0.0.1:8080.evil.example/',
		'http://127.0.0.1:8080@evil.example/den',
		'https://127.0.0.1:8080/den',
		'/den',
		'not a url',
		'http://[::1'
	])('asks "Which page?" for a Referer of %j', async (referer) => {
		const data = await loadWith({ referer });

		expect(data.offer.page).toStrictEqual({ kind: 'ask' });
		expect(data.offer.pageType).toBeUndefined();
	});

	it('asks "Which page?" when the Referer is the report page itself, as after a post without script', async () => {
		const data = await loadWith({ referer: `${FIRST_PARTY}/report-a-bug` });

		expect(data.offer.page).toStrictEqual({ kind: 'ask' });
		expect(data.offer.pageType).toBeUndefined();
	});

	it('asks "Which page?" with no Referer at all', async () => {
		const data = await loadWith({});

		expect(data.offer).toStrictEqual({
			page: { kind: 'ask' },
			pageType: undefined,
			browser: undefined
		});
	});

	it('offers the browser from the User-Agent, cut to fit', async () => {
		const short = await loadWith({ 'user-agent': 'Mozilla/5.0' });
		const long = await loadWith({ 'user-agent': 'M'.repeat(2000) });

		expect(short.offer.browser).toBe('Mozilla/5.0');
		expect(utf8ByteLength(long.offer.browser ?? '')).toBe(BROWSER_MAX_BYTES);
	});

	it('gives the frame its path', async () => {
		const data = await loadWith({});

		expect(data.trail).toStrictEqual([{ step: 'named', label: 'Report a bug', href: undefined }]);
	});
});
