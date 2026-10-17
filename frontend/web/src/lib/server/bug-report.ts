/**
 * Filing a bug report, below the seam: the outcome program over the
 * {@link ZurfurApi} port, the web's own build id, whether the web runs on
 * the dev profile's test stack, and the one way an address becomes the
 * report's page (judged against the configured first party, never the
 * request's own address).
 */

import { dev } from '$app/environment';
import { env } from '$env/dynamic/private';
import { Effect } from 'effect';
import { ARCHIVED_SUFFIX } from '$lib/api/bug-report';
import { HttpStatus } from '$lib/api/http-status';
import type { BugReference, FirstPartyOrigin } from '$lib/types/brand';
import {
	pageTypeOf,
	parsePage,
	parseVersion,
	refusedCharacter,
	type BugPage,
	type PageType
} from '$lib/types/bug-report-rules';
import { ZurfurApi, type BugReportFiling } from './api/zurfur-api';
import { denQueryFromSearch } from './den/den-path';

/** The build id the web reports when ZURFUR_BUILD_ID is unset or doesn't fit the version rule. */
export const UNKNOWN_BUILD_ID = 'unknown';

/**
 * The run-time environment, where any variable may be unset. `svelte-kit sync`
 * types a variable it finds set as always present, so reads go through this view.
 */
function runtimeEnv(): Readonly<Record<string, string | undefined>> {
	return env;
}

/** The web's own build, read from ZURFUR_BUILD_ID at each call: {@link UNKNOWN_BUILD_ID} when unset or unfit. */
export function webBuildId(): string {
	const configured = parseVersion(runtimeEnv().ZURFUR_BUILD_ID ?? '');
	return configured.kind === 'accepted' ? configured.value : UNKNOWN_BUILD_ID;
}

/** The value of ZURFUR_WEB_STACK that `just dev-profile-web` sets. */
export const DEV_PROFILE_STACK = 'dev-profile';

/** Whether the web runs on the dev profile's test stack: ZURFUR_WEB_STACK=dev-profile, in a dev build only, read at each call. */
export function onDevProfileStack(): boolean {
	return dev && runtimeEnv().ZURFUR_WEB_STACK === DEV_PROFILE_STACK;
}

/** An address on this site, reduced to what a report keeps. */
interface SitePath {
	readonly pathname: string;
	readonly archived: boolean;
}

/**
 * `text` as an address on `firstParty`: an absolute address whose origin is
 * exactly `firstParty`, or (when `pathAllowed`) a path starting with `/` that
 * stays on it. The query and fragment are dropped; `archived` says the query
 * asked for `includeDeleted=true` (the Den's own rule).
 */
function sitePathOf(
	text: string,
	firstParty: FirstPartyOrigin | undefined,
	pathAllowed: boolean
): SitePath | undefined {
	if (firstParty === undefined) return undefined;
	const isPath = text.startsWith('/');
	if (isPath && !pathAllowed) return undefined;
	const url = (isPath ? URL.parse(text, firstParty) : URL.parse(text)) ?? undefined;
	if (url?.origin !== firstParty) return undefined;
	return { pathname: url.pathname, archived: denQueryFromSearch(url.searchParams).includeDeleted };
}

/** `site` as the report's page: its path, with {@link ARCHIVED_SUFFIX} when archived, if it fits the page rule. */
function bugPageOf(site: SitePath): BugPage | undefined {
	const parsed = parsePage(`${site.pathname}${site.archived ? ARCHIVED_SUFFIX : ''}`);
	return parsed.kind === 'accepted' ? parsed.value : undefined;
}

/** The page the reporter came from, read off a `Referer` on this site, and its page type. */
export interface RefererPage {
	readonly page: BugPage;
	readonly pageType: PageType;
}

/**
 * The page a `Referer` names, when its origin is exactly `firstParty` and it
 * isn't the report page itself (a post without script comes back from
 * there); otherwise `undefined`, and the report page asks "Which page?".
 */
export function pageOfReferer(
	referer: string | undefined,
	firstParty: FirstPartyOrigin | undefined
): RefererPage | undefined {
	if (referer === undefined) return undefined;
	const site = sitePathOf(referer, firstParty, false);
	if (site === undefined) return undefined;
	const pageType = pageTypeOf(site.pathname);
	if (pageType === 'Report a bug') return undefined;
	const page = bugPageOf(site);
	return page === undefined ? undefined : { page, pageType };
}

/** How a posted page is judged. */
export type PostedPage =
	| { readonly kind: 'accepted'; readonly page: BugPage }
	/** Empty after trimming: not sent. */
	| { readonly kind: 'notSent' }
	/** It holds a control or bidirectional character. */
	| { readonly kind: 'hiddenCharacter' }
	/** Neither an address on this site nor a path, or too long once reduced. */
	| { readonly kind: 'notAnAddress' };

/**
 * A posted page ("Which page?", or the page the report page offered):
 * trimmed; refused for a hidden character before anything reads it as an
 * address; then an address on `firstParty` or a path, reduced to its path,
 * keeping {@link ARCHIVED_SUFFIX} when the text ends with it or its query
 * asks for archived items.
 */
export function judgePostedPage(
	text: string,
	firstParty: FirstPartyOrigin | undefined
): PostedPage {
	const trimmed = text.trim();
	if (trimmed.length === 0) return { kind: 'notSent' };
	if (refusedCharacter('page', trimmed) !== undefined) return { kind: 'hiddenCharacter' };
	const suffixed = trimmed.endsWith(ARCHIVED_SUFFIX);
	const address = suffixed ? trimmed.slice(0, -ARCHIVED_SUFFIX.length) : trimmed;
	const site = sitePathOf(address, firstParty, true);
	if (site === undefined) return { kind: 'notAnAddress' };
	const page = bugPageOf({ pathname: site.pathname, archived: site.archived || suffixed });
	return page === undefined ? { kind: 'notAnAddress' } : { kind: 'accepted', page };
}

/** How filing a report comes back. */
export type BugReportOutcome =
	| { readonly kind: 'sent'; readonly reference: BugReference }
	/** The hourly limit is reached (429). */
	| { readonly kind: 'limit' }
	/** The session ended (401): sign in again. */
	| { readonly kind: 'signedOut' }
	/** Refused (422, 403, …), failed (5xx), unreachable, or out of time. */
	| { readonly kind: 'couldNotSend' };

const LIMIT: BugReportOutcome = { kind: 'limit' };
const SIGNED_OUT: BugReportOutcome = { kind: 'signedOut' };
const COULD_NOT_SEND: BugReportOutcome = { kind: 'couldNotSend' };

/** The outcome an answer's status means when it isn't a success. */
function outcomeOfStatus(status: number): BugReportOutcome {
	if (status === HttpStatus.TooManyRequests) return LIMIT;
	if (status === HttpStatus.Unauthorized) return SIGNED_OUT;
	return COULD_NOT_SEND;
}

/** File `filing`; every failure becomes an outcome, none is left in the error channel. */
export function fileBugReportOutcome(
	filing: BugReportFiling
): Effect.Effect<BugReportOutcome, never, ZurfurApi> {
	const filed = Effect.gen(function* () {
		const api = yield* ZurfurApi;
		const reference = yield* api.fileBugReport(filing);
		return { kind: 'sent', reference } satisfies BugReportOutcome;
	});
	return filed.pipe(
		Effect.catchTags({
			NotAuthenticated: () => Effect.succeed(SIGNED_OUT),
			ApiProblem: ({ problem }) => Effect.succeed(outcomeOfStatus(problem.status)),
			ContractViolation: ({ status }) => Effect.succeed(outcomeOfStatus(status)),
			NetworkFailure: () => Effect.succeed(COULD_NOT_SEND)
		})
	);
}
