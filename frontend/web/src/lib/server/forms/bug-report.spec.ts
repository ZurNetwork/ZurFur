import { describe, expect, it, vi } from 'vitest';
import { DISPLAY_MATCHED, DISPLAY_UNMATCHED, TICKED } from '$lib/api/bug-report';
import { firstPartyOrigin } from '$lib/types/brand';

vi.mock('$env/dynamic/private', () => ({ env: {} }));

const { echoOf, postedBugReport, readBugReportForm } = await import('./bug-report');
type Posted = Parameters<typeof readBugReportForm>[0];

const CONTEXT = { firstParty: firstPartyOrigin('http://127.0.0.1:8080'), buildId: 'build-9' };
const EMOJI = '😀';

/** The filing `posted` reads as, or the refusal. */
function read(posted: Posted) {
	return readBugReportForm(posted, CONTEXT);
}

/** A filing of `description` alone. */
function bare(description: string) {
	return {
		kind: 'filing',
		filing: {
			description,
			page: undefined,
			pageType: undefined,
			webVersion: undefined,
			browser: undefined,
			window: undefined,
			display: undefined,
			errorStatus: undefined
		}
	};
}

describe('postedBugReport', () => {
	it('keeps the known text fields only, never a file or an unknown field', () => {
		const form = new FormData();
		form.append('description', 'Broken.');
		form.append('page', new Blob(['x']), 'page.txt');
		form.append('reporter', 'did:plc:mallory');

		expect(postedBugReport(form)).toStrictEqual({ description: 'Broken.' });
	});

	it('echoes the description and page as posted, empty when absent', () => {
		expect(echoOf({ description: ' a\r\n', page: '/x?y' })).toStrictEqual({
			description: ' a\r\n',
			page: '/x?y'
		});
		expect(echoOf({})).toStrictEqual({ description: '', page: '' });
	});
});

describe('readBugReportForm: the description', () => {
	it('counts code points, never UTF-16 units: 4,000 emoji pass, 4,001 do not', () => {
		expect(read({ description: EMOJI.repeat(4000) })).toStrictEqual(bare(EMOJI.repeat(4000)));
		expect(read({ description: EMOJI.repeat(4001) })).toStrictEqual({
			kind: 'refused',
			refusal: { kind: 'descriptionTooLong' }
		});
	});

	it('turns CR LF into LF and trims', () => {
		expect(read({ description: '  one\r\ntwo\r\n ' })).toStrictEqual(bare('one\ntwo'));
	});

	it('keeps tabs and line feeds', () => {
		expect(read({ description: 'a\tb\nc' })).toStrictEqual(bare('a\tb\nc'));
	});
});

describe('readBugReportForm: the tick boxes', () => {
	it('stamps the build it is given for a ticked version, whatever else was posted', () => {
		const posted = { description: 'x', sendWebVersion: TICKED, webVersion: 'forged' } as Posted;
		const outcome = read(posted);
		expect(outcome.kind === 'filing' && outcome.filing.webVersion).toBe('build-9');
	});

	it('counts a box as ticked only when it posts "on"', () => {
		expect(read({ description: 'x', sendPage: 'true', page: '/den' })).toStrictEqual(bare('x'));
		expect(read({ description: 'x', sendPage: '', page: '/den' })).toStrictEqual(bare('x'));
	});

	it('reads a ticked window, display and error status', () => {
		const outcome = read({
			description: 'x',
			sendWindow: TICKED,
			windowWidth: '1',
			windowHeight: '100000',
			sendDisplay: TICKED,
			reducedMotion: DISPLAY_UNMATCHED,
			moreContrast: DISPLAY_MATCHED,
			forcedColors: DISPLAY_MATCHED,
			sendErrorStatus: TICKED,
			errorStatus: '599'
		});

		expect(outcome).toStrictEqual({
			kind: 'filing',
			filing: {
				...bare('x').filing,
				window: { width: 1, height: 100000 },
				display: { reducedMotion: false, moreContrast: true, forcedColors: true },
				errorStatus: 599
			}
		});
	});

	it.each(['0', '100001', '-1', '1e3', '12.5', ' 12', '012', '', 'wide'])(
		'refuses a window side of %j',
		(side) => {
			expect(
				read({ description: 'x', sendWindow: TICKED, windowWidth: side, windowHeight: '10' })
			).toStrictEqual({ kind: 'refused', refusal: { kind: 'itemRefused', item: 'window' } });
		}
	);

	it.each(['399', '600', '200', '4040', '', 'x'])('refuses an error status of %j', (status) => {
		expect(read({ description: 'x', sendErrorStatus: TICKED, errorStatus: status })).toStrictEqual({
			kind: 'refused',
			refusal: { kind: 'itemRefused', item: 'errorStatus' }
		});
	});

	it.each(['on', 'TRUE', '1', ''])('refuses a display preference of %j', (value) => {
		expect(
			read({
				description: 'x',
				sendDisplay: TICKED,
				reducedMotion: value,
				moreContrast: DISPLAY_UNMATCHED,
				forcedColors: DISPLAY_UNMATCHED
			})
		).toStrictEqual({ kind: 'refused', refusal: { kind: 'itemRefused', item: 'display' } });
	});

	it('takes the page type from the closed list only', () => {
		const known = read({ description: 'x', sendPageType: TICKED, pageType: ' An Account ' });
		const unknown = read({ description: 'x', sendPageType: TICKED, pageType: 'Admin' });

		expect(known.kind === 'filing' && known.filing.pageType).toBe('An Account');
		expect(unknown).toStrictEqual({
			kind: 'refused',
			refusal: { kind: 'itemRefused', item: 'pageType' }
		});
	});

	it('names the description first when several items are refused', () => {
		const outcome = read({ description: '', sendPage: TICKED, page: 'https://evil.example/' });
		expect(outcome).toStrictEqual({ kind: 'refused', refusal: { kind: 'descriptionEmpty' } });
	});
});
