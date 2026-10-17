/**
 * The bug-report form as an Effect Schema: posted fields in, the port's
 * {@link BugReportFiling} out. Two steps: the tick boxes choose which items
 * go (an unticked item is dropped before anything reads its value, and the
 * version is always the server's own build), then each chosen item is
 * parsed by the shared rules (`bug-report-rules.ts`), counting characters as
 * code points and bytes as UTF-8, never UTF-16 units.
 */

import { Either, ParseResult, Schema, type SchemaAST } from 'effect';
import {
	BUG_REPORT_FIELDS,
	BUG_REPORT_ITEMS,
	DISPLAY_MATCHED,
	DISPLAY_UNMATCHED,
	TICKED,
	type BugReportEcho,
	type BugReportFieldName,
	type BugReportItem,
	type BugReportRefusalKind
} from '$lib/api/bug-report';
import type { FirstPartyOrigin } from '$lib/types/brand';
import {
	PAGE_TYPES,
	parseBrowser,
	parseDescription,
	parsePage,
	parsePageTypeField,
	parseVersion,
	type BugBrowser,
	type BugDescription,
	type BugPage,
	type BugVersion,
	type FieldParse,
	type OptionalFieldParse
} from '$lib/types/bug-report-rules';
import type { BugReportFiling } from '../api/zurfur-api';
import { judgePostedPage } from '../bug-report';

/** What a post carried: each known field's text, absent when not posted (or not text). */
export type PostedBugReport = Readonly<Partial<Record<BugReportFieldName, string>>>;

/** The known fields' text off `form`; any other field, and any file, is left behind. */
export function postedBugReport(form: FormData): PostedBugReport {
	const posted: Partial<Record<BugReportFieldName, string>> = {};
	for (const name of Object.values(BUG_REPORT_FIELDS)) {
		const value = form.get(name);
		if (typeof value === 'string') posted[name] = value;
	}
	return posted;
}

/** The reporter's own text in `posted`, to carry back on a failure. */
export function echoOf(posted: PostedBugReport): BugReportEcho {
	return { description: posted.description ?? '', page: posted.page ?? '' };
}

/** What the form is read against: the site's own origin and the web's own build. */
export interface BugReportFormContext {
	readonly firstParty: FirstPartyOrigin | undefined;
	readonly buildId: string;
}

/** Why a field was refused, as the issue's message: the step after maps it to a failure kind. */
type RefusalCode = 'empty' | 'tooLong' | 'hiddenCharacter' | 'notAnAddress' | 'refused';

const POSTED_FIELD = Schema.optional(Schema.String);

/** The post as it arrives: every known field optional text; unknown fields are ignored. */
const Posted = Schema.Struct({
	description: POSTED_FIELD,
	sendPage: POSTED_FIELD,
	page: POSTED_FIELD,
	sendPageType: POSTED_FIELD,
	pageType: POSTED_FIELD,
	sendWebVersion: POSTED_FIELD,
	sendBrowser: POSTED_FIELD,
	browser: POSTED_FIELD,
	sendWindow: POSTED_FIELD,
	windowWidth: POSTED_FIELD,
	windowHeight: POSTED_FIELD,
	sendDisplay: POSTED_FIELD,
	reducedMotion: POSTED_FIELD,
	moreContrast: POSTED_FIELD,
	forcedColors: POSTED_FIELD,
	sendErrorStatus: POSTED_FIELD,
	errorStatus: POSTED_FIELD
});

const CHOSEN_TEXT = Schema.UndefinedOr(Schema.String);

/** The items the reporter chose, still as text; an unticked item is `undefined`. */
const Chosen = Schema.Struct({
	description: Schema.String,
	page: CHOSEN_TEXT,
	pageType: CHOSEN_TEXT,
	webVersion: CHOSEN_TEXT,
	browser: CHOSEN_TEXT,
	window: Schema.UndefinedOr(Schema.Struct({ width: Schema.String, height: Schema.String })),
	display: Schema.UndefinedOr(
		Schema.Struct({
			reducedMotion: Schema.String,
			moreContrast: Schema.String,
			forcedColors: Schema.String
		})
	),
	errorStatus: CHOSEN_TEXT
});

type ChosenItems = typeof Chosen.Type;

/** `value` when `box` is ticked, else `undefined`: an unticked item's value is never read. */
function ifTicked<T>(box: string | undefined, value: () => T): T | undefined {
	return box === TICKED ? value() : undefined;
}

/** The ticked items of `posted`; the version, when ticked, is `buildId`, never anything posted. */
function chosenItems(posted: typeof Posted.Type, buildId: string): ChosenItems {
	return {
		description: posted.description ?? '',
		page: ifTicked(posted.sendPage, () => posted.page ?? ''),
		pageType: ifTicked(posted.sendPageType, () => posted.pageType ?? ''),
		webVersion: ifTicked(posted.sendWebVersion, () => buildId),
		browser: ifTicked(posted.sendBrowser, () => posted.browser ?? ''),
		window: ifTicked(posted.sendWindow, () => ({
			width: posted.windowWidth ?? '',
			height: posted.windowHeight ?? ''
		})),
		display: ifTicked(posted.sendDisplay, () => ({
			reducedMotion: posted.reducedMotion ?? '',
			moreContrast: posted.moreContrast ?? '',
			forcedColors: posted.forcedColors ?? ''
		})),
		errorStatus: ifTicked(posted.sendErrorStatus, () => posted.errorStatus ?? '')
	};
}

/** The tick-box step: decode only, a form is never encoded back. */
function chooseItems(buildId: string) {
	return Schema.transformOrFail(Posted, Chosen, {
		strict: true,
		decode: (posted) => ParseResult.succeed(chosenItems(posted, buildId)),
		encode: (chosen, _options, ast) =>
			ParseResult.fail(new ParseResult.Forbidden(ast, chosen, 'a form is only decoded'))
	});
}

/** A type guard from a rule's parser: `text` is the brand when the rule accepts it unchanged. */
function acceptedAsIs<T extends string>(
	parse: (text: string) => FieldParse<T> | OptionalFieldParse<T>
): (text: string) => text is T {
	return (text): text is T => {
		const parsed = parse(text);
		return parsed.kind === 'accepted' && parsed.value === text;
	};
}

/** Each brand as a schema of its own type: the rule applied by `Schema.filter`. */
const Description = Schema.typeSchema(
	Schema.String.pipe(Schema.filter(acceptedAsIs(parseDescription)))
);
const Page = Schema.typeSchema(Schema.String.pipe(Schema.filter(acceptedAsIs(parsePage))));
const Version = Schema.typeSchema(Schema.String.pipe(Schema.filter(acceptedAsIs(parseVersion))));
const Browser = Schema.typeSchema(Schema.String.pipe(Schema.filter(acceptedAsIs(parseBrowser))));

/** A refusal of `actual` with `code` as its message. */
function refusal(ast: SchemaAST.AST, actual: unknown, code: RefusalCode) {
	return ParseResult.fail(new ParseResult.Type(ast, actual, code));
}

/** The description: CR LF to LF, trimmed, then the description rule. */
const descriptionField = Schema.transformOrFail(Schema.String, Description, {
	strict: true,
	decode: (text, _options, ast) => {
		const parsed = parseDescription(text);
		if (parsed.kind === 'accepted') return ParseResult.succeed<BugDescription>(parsed.value);
		switch (parsed.refusal.kind) {
			case 'empty':
				return refusal(ast, text, 'empty');
			case 'refusedCharacter':
				return refusal(ast, text, 'hiddenCharacter');
			case 'tooLong':
				return refusal(ast, text, 'tooLong');
			case 'notAPath':
			case 'queryOrFragment':
			case 'unknownPageType':
				return refusal(ast, text, 'refused');
		}
	},
	encode: ParseResult.succeed
});

/** An optional item: `undefined` (unticked) or empty after trimming is not sent; otherwise `parse` decides. */
function optionalTextField<T extends string>(
	target: Schema.Schema<T>,
	parse: (text: string) => OptionalFieldParse<T>
) {
	return Schema.transformOrFail(CHOSEN_TEXT, Schema.UndefinedOr(target), {
		strict: true,
		decode: (text, _options, ast) => {
			if (text === undefined) return ParseResult.succeed(undefined);
			const parsed = parse(text);
			switch (parsed.kind) {
				case 'accepted':
					return ParseResult.succeed(parsed.value);
				case 'notSent':
					return ParseResult.succeed(undefined);
				case 'refused':
					return refusal(
						ast,
						text,
						parsed.refusal.kind === 'refusedCharacter' ? 'hiddenCharacter' : 'refused'
					);
			}
		},
		encode: ParseResult.succeed
	});
}

/** The page: an address on the site or a path, judged against `firstParty`, reduced to its path. */
function pageField(firstParty: FirstPartyOrigin | undefined) {
	return Schema.transformOrFail(CHOSEN_TEXT, Schema.UndefinedOr(Page), {
		strict: true,
		decode: (text, _options, ast) => {
			if (text === undefined) return ParseResult.succeed(undefined);
			const judged = judgePostedPage(text, firstParty);
			switch (judged.kind) {
				case 'accepted':
					return ParseResult.succeed<BugPage>(judged.page);
				case 'notSent':
					return ParseResult.succeed(undefined);
				case 'hiddenCharacter':
					return refusal(ast, text, 'hiddenCharacter');
				case 'notAnAddress':
					return refusal(ast, text, 'notAnAddress');
			}
		},
		encode: ParseResult.succeed
	});
}

const pageTypeField = optionalTextField(Schema.Literal(...PAGE_TYPES), parsePageTypeField);
const versionField = optionalTextField<BugVersion>(Version, parseVersion);
const browserField = optionalTextField<BugBrowser>(Browser, parseBrowser);

/** A whole number from `low` to `high`, written plainly in decimal (no sign, exponent or leading zero). */
function boundedInteger(low: number, high: number) {
	return Schema.transformOrFail(Schema.String, Schema.Int.pipe(Schema.between(low, high)), {
		strict: true,
		decode: (text, _options, ast) =>
			/^[1-9][0-9]{0,8}$/.test(text)
				? ParseResult.succeed(Number(text))
				: refusal(ast, text, 'refused'),
		encode: (value) => ParseResult.succeed(String(value))
	});
}

const WINDOW_SIDE_MAX = 100_000;
const ERROR_STATUS_MIN = 400;
const ERROR_STATUS_MAX = 599;

const windowField = Schema.UndefinedOr(
	Schema.Struct({
		width: boundedInteger(1, WINDOW_SIDE_MAX),
		height: boundedInteger(1, WINDOW_SIDE_MAX)
	})
);

/** A display preference: exactly {@link DISPLAY_MATCHED} or {@link DISPLAY_UNMATCHED}. */
const displayPreference = Schema.transformLiterals(
	[DISPLAY_MATCHED, true],
	[DISPLAY_UNMATCHED, false]
);

const displayField = Schema.UndefinedOr(
	Schema.Struct({
		reducedMotion: displayPreference,
		moreContrast: displayPreference,
		forcedColors: displayPreference
	})
);

const errorStatusField = Schema.UndefinedOr(boundedInteger(ERROR_STATUS_MIN, ERROR_STATUS_MAX));

/** The chosen items, each parsed by its rule. */
function parsedItems(firstParty: FirstPartyOrigin | undefined) {
	return Schema.Struct({
		description: descriptionField,
		page: pageField(firstParty),
		pageType: pageTypeField,
		webVersion: versionField,
		browser: browserField,
		window: windowField,
		display: displayField,
		errorStatus: errorStatusField
	});
}

/** The whole form: the tick boxes choose, then the rules parse. */
export function bugReportForm(context: BugReportFormContext) {
	return Schema.compose(chooseItems(context.buildId), parsedItems(context.firstParty));
}

/** How reading the form came out: the filing to send, or why it was refused. */
export type BugReportFormRead =
	| { readonly kind: 'filing'; readonly filing: BugReportFiling }
	| { readonly kind: 'refused'; readonly refusal: BugReportRefusalKind };

const REFUSAL_CODES: ReadonlyMap<string, RefusalCode> = new Map<string, RefusalCode>([
	['empty', 'empty'],
	['tooLong', 'tooLong'],
	['hiddenCharacter', 'hiddenCharacter'],
	['notAnAddress', 'notAnAddress']
]);

/** The item an issue's path names, when it names one. */
function itemOf(path: readonly PropertyKey[]): BugReportItem | undefined {
	const [head] = path;
	return BUG_REPORT_ITEMS.find((item) => item === head);
}

/** The failure kind for a refused `item`, from its issue's message. */
function refusalKindOf(item: BugReportItem | undefined, message: string): BugReportRefusalKind {
	const code = REFUSAL_CODES.get(message) ?? 'refused';
	switch (item) {
		case 'description':
			if (code === 'empty') return { kind: 'descriptionEmpty' };
			if (code === 'hiddenCharacter') return { kind: 'hiddenCharacter', item };
			if (code === 'tooLong') return { kind: 'descriptionTooLong' };
			return { kind: 'itemRefused', item };
		case 'page':
			if (code === 'hiddenCharacter') return { kind: 'hiddenCharacter', item };
			return { kind: 'pageAddress' };
		case 'browser':
			if (code === 'hiddenCharacter') return { kind: 'hiddenCharacter', item };
			return { kind: 'itemRefused', item };
		case 'pageType':
		case 'webVersion':
		case 'window':
		case 'display':
		case 'errorStatus':
			return { kind: 'itemRefused', item };
		case undefined:
			return { kind: 'itemRefused', item: 'description' };
	}
}

/** Read `posted` through {@link bugReportForm}: the first refusal names its item. */
export function readBugReportForm(
	posted: PostedBugReport,
	context: BugReportFormContext
): BugReportFormRead {
	const decoded = Schema.decodeUnknownEither(bugReportForm(context), { errors: 'first' })(posted);
	if (Either.isRight(decoded)) return { kind: 'filing', filing: decoded.right };
	const [issue] = ParseResult.ArrayFormatter.formatErrorSync(decoded.left);
	const refusalKind = refusalKindOf(itemOf(issue?.path ?? []), issue?.message ?? '');
	return { kind: 'refused', refusal: refusalKind };
}
