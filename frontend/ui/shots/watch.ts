/**
 * The checks every screenshot test runs before its picture and again after
 * it: no request leaves the loopback server, no page throws, no page builds a
 * SharedWorker, RTCPeerConnection or WebTransport, no play function throws,
 * and Storybook rendered without its error display; and the walk of one
 * entry, which runs them. The walker and the self-tests share them from here.
 */
import { readFileSync, rmdirSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test, type BrowserContext, type Frame, type Page } from '@playwright/test';
import { entryAddress, isShot, type Entry, type Skin, type ViewMode } from './storybook.ts';

/** The loopback port the static server listens on inside the container. */
export const shotsPort = 5199;

/** The only origin a page may load from. */
export const shotsOrigin = `http://127.0.0.1:${String(shotsPort)}`;

/** True when a page may load `url`: the loopback server's origin, or a `data:` or `blob:` URL. */
export function isAllowedUrl(url: string): boolean {
	let parsed: URL;
	try {
		parsed = new URL(url);
	} catch {
		return false;
	}
	if (parsed.protocol === 'data:' || parsed.protocol === 'blob:') return true;
	return parsed.origin === shotsOrigin;
}

/** What went wrong in one test's pages, collected as it happens. */
export interface Watch {
	readonly problems: Set<string>;
}

function watchPage(page: Page, watch: Watch): void {
	page.on('pageerror', (error) => {
		watch.problems.add(`page error: ${error.name}: ${error.message}`);
	});
	page.on('websocket', (socket) => {
		if (!isAllowedUrl(socket.url())) watch.problems.add(`websocket to ${socket.url()}`);
	});
}

/**
 * The constructors whose connections the request watch can't see (a shared
 * worker's own requests, WebRTC's STUN and TURN, WebTransport's QUIC): each
 * construction in a page is a problem.
 */
export const watchedConstructors = [
	'SharedWorker',
	'RTCPeerConnection',
	'webkitRTCPeerConnection',
	'WebTransport'
] as const;

/** The binding the init script reports a watched construction through. */
const constructionBinding = '__zurfurShotsConstructed';

/**
 * Runs in every frame before its scripts: wraps each watched constructor the
 * frame has so that every construction calls the binding with its name first.
 */
function reportConstructions([binding, names]: readonly [string, readonly string[]]): void {
	for (const name of names) {
		const original: unknown = Reflect.get(globalThis, name);
		if (typeof original !== 'function') continue;
		const reporting = new Proxy(original, {
			construct(target, args, newTarget) {
				const report: unknown = Reflect.get(globalThis, binding);
				if (typeof report === 'function') Reflect.apply(report, globalThis, [name]);
				return Reflect.construct(target, args, newTarget);
			}
		});
		Reflect.defineProperty(globalThis, name, { value: reporting });
	}
}

/**
 * Watches every page of the test's context: each request not to the loopback
 * server is aborted and recorded, as is every page error, off-host websocket
 * and construction of a watched constructor, in every frame. Call it before
 * the first navigation.
 */
export async function watchContext(context: BrowserContext, page: Page): Promise<Watch> {
	const watch: Watch = { problems: new Set() };
	await context.exposeBinding(constructionBinding, (_source, name: unknown) => {
		const watched = watchedConstructors.find((known) => known === name);
		watch.problems.add(`construction of ${watched ?? 'an unnamed constructor'}`);
	});
	await context.route('**/*', async (route) => {
		const url = route.request().url();
		if (isAllowedUrl(url)) {
			await route.continue();
			return;
		}
		watch.problems.add(`request to ${url}`);
		await route.abort('blockedbyclient');
	});
	await context.routeWebSocket(/.*/, (socket) => {
		if (!isAllowedUrl(socket.url())) watch.problems.add(`websocket to ${socket.url()}`);
		void socket.close();
	});
	context.on('request', (request) => {
		if (!isAllowedUrl(request.url())) watch.problems.add(`request to ${request.url()}`);
	});
	for (const open of context.pages()) watchPage(open, watch);
	context.on('page', (opened) => {
		watchPage(opened, watch);
	});
	if (!context.pages().includes(page)) watchPage(page, watch);
	await context.addInitScript(reportConstructions, [
		constructionBinding,
		watchedConstructors
	] as const);
	await context.addInitScript(listenToStorybook, [docsRenderedFlag, playFailuresKey] as const);
	return watch;
}

/** Fails the test when anything went wrong so far; `moment` names the check in the failure. */
export function expectNoProblems(watch: Watch, moment: string): void {
	expect(
		[...watch.problems],
		`requests off 127.0.0.1, page errors, websockets, watched constructions or play function failures, ${moment}`
	).toEqual([]);
}

/** The flag the init script sets once Storybook says a docs page rendered. */
const docsRenderedFlag = '__zurfurShotsDocsRendered';

/** The list the init script adds each play function's failure to, as `<name>: <message>`. */
const playFailuresKey = '__zurfurShotsPlayFailures';

/**
 * Runs in every frame before its scripts: catches Storybook's channel as it is
 * installed, sets a flag on its `docsRendered` event and records each
 * `playFunctionThrewException`. Storybook goes on to `finished` after a play
 * function throws, without its error display, so the record is the only sign.
 */
function listenToStorybook([flag, failuresKey]: readonly [string, string]): void {
	let channel: unknown;
	const failures: string[] = [];
	Reflect.set(globalThis, flag, false);
	Reflect.set(globalThis, failuresKey, failures);
	Reflect.defineProperty(globalThis, '__STORYBOOK_ADDONS_CHANNEL__', {
		configurable: true,
		get: () => channel,
		set: (next: unknown) => {
			channel = next;
			const on: unknown = Object(next) === next ? Reflect.get(Object(next), 'on') : undefined;
			if (typeof on !== 'function') return;
			Reflect.apply(on, next, [
				'docsRendered',
				() => {
					Reflect.set(globalThis, flag, true);
				}
			]);
			Reflect.apply(on, next, [
				'playFunctionThrewException',
				(error: unknown) => {
					const field = (key: string): string => {
						const value: unknown =
							Object(error) === error ? Reflect.get(Object(error), key) : undefined;
						return typeof value === 'string' ? value : '';
					};
					const [firstLine = ''] = field('message').split('\n');
					failures.push(`${field('name') || 'Error'}: ${firstLine}`);
				}
			]);
		}
	});
}

/** The play function failures the page recorded so far. */
export async function playFailuresIn(target: Page | Frame): Promise<readonly string[]> {
	const recorded: unknown = await target.evaluate(
		(key) => Reflect.get(globalThis, key),
		playFailuresKey
	);
	return Array.isArray(recorded)
		? recorded.filter((failure: unknown) => typeof failure === 'string')
		: [];
}

/** Where Storybook's preview ended up. */
export type RenderState = 'rendered' | 'error-display' | 'no-preview' | 'play-failed';

/**
 * Reads the preview's state in the page: Storybook's error display or its
 * no-preview screen, by the body's class; a play function that threw, by the
 * init script's record; otherwise rendered once every story render reached its
 * `finished` phase (play functions and afterEach included) and, on a docs page,
 * once Storybook said the page rendered. False while none of these.
 */
function renderStateInPage([viewMode, flag, failuresKey]: readonly [ViewMode, string, string]):
	RenderState | false {
	const body = document.querySelector('body') ?? undefined;
	if (body === undefined) return false;
	if (body.classList.contains('sb-show-errordisplay')) return 'error-display';
	if (body.classList.contains('sb-show-nopreview')) return 'no-preview';
	const failures: unknown = Reflect.get(globalThis, failuresKey);
	if (Array.isArray(failures) && failures.length > 0) return 'play-failed';
	const preview: unknown = Reflect.get(globalThis, '__STORYBOOK_PREVIEW__');
	const renders: unknown =
		Object(preview) === preview ? Reflect.get(Object(preview), 'storyRenders') : undefined;
	const phases = Array.isArray(renders)
		? renders.map((render: unknown) =>
				Object(render) === render ? Reflect.get(Object(render), 'phase') : undefined
			)
		: [];
	const allFinished = phases.every((phase) => phase === 'finished');
	if (viewMode === 'story') return phases.length > 0 && allFinished ? 'rendered' : false;
	return Reflect.get(globalThis, flag) === true && allFinished ? 'rendered' : false;
}

/** Waits until the preview rendered, showed its error display or a play function threw, and says which. */
export async function waitForStorybook(
	target: Page | Frame,
	viewMode: ViewMode
): Promise<RenderState> {
	const handle = await target.waitForFunction(renderStateInPage, [
		viewMode,
		docsRenderedFlag,
		playFailuresKey
	] as const);
	const state: unknown = await handle.jsonValue();
	await handle.dispose();
	return state === 'error-display' || state === 'no-preview' || state === 'play-failed'
		? state
		: 'rendered';
}

/** Checks the preview's state, naming each play function failure when there is one. */
async function expectState(page: Page, viewMode: ViewMode, moment: string): Promise<void> {
	const state = await waitForStorybook(page, viewMode);
	const failures = state === 'play-failed' ? await playFailuresIn(page) : [];
	expect({ state, failures }, moment).toEqual({ state: 'rendered', failures: [] });
}

/** Waits for the fonts, the images that load now, and a quiet network. */
export async function settle(page: Page): Promise<void> {
	await page.evaluate(async () => {
		await document.fonts.ready;
	});
	await page.waitForFunction(() =>
		Array.from(document.images).every((image) => image.complete || image.loading === 'lazy')
	);
	await page.waitForLoadState('networkidle');
}

/** The globals the preview applied, read from its store. */
function globalsInPage(): unknown {
	const preview: unknown = Reflect.get(globalThis, '__STORYBOOK_PREVIEW__');
	const store: unknown =
		Object(preview) === preview ? Reflect.get(Object(preview), 'storyStoreValue') : undefined;
	const globals: unknown =
		Object(store) === store ? Reflect.get(Object(store), 'userGlobals') : undefined;
	return Object(globals) === globals ? Reflect.get(Object(globals), 'globals') : undefined;
}

/**
 * Checks an opened entry rendered in the asked skin with motion stopped:
 * Storybook's error display, its no-preview screen, a play function that threw
 * or a global the address did not set each fail the test.
 */
export async function expectRendered(page: Page, viewMode: ViewMode, skin: Skin): Promise<void> {
	await expectState(page, viewMode, "Storybook's preview state");
	await settle(page);
	await expectState(page, viewMode, "Storybook's preview state, settled");
	expect(await page.evaluate(globalsInPage), 'the globals the address set').toMatchObject({
		skin,
		motion: 'still'
	});
}

/** A file's bytes, or undefined when it can't be read. */
function bytesOf(file: string): Buffer | undefined {
	try {
		return readFileSync(file);
	} catch {
		return undefined;
	}
}

/**
 * Puts a reference picture back as it was before the picture was taken:
 * removed (with its folder, once empty) when there was none, its old bytes
 * when they changed, and left untouched otherwise, so a plain run writes
 * nothing.
 */
export function putPictureBack(file: string, before: Buffer | undefined): void {
	const now = bytesOf(file);
	if (before !== undefined) {
		if (now === undefined || !now.equals(before)) writeFileSync(file, before);
		return;
	}
	if (now === undefined) return;
	rmSync(file);
	try {
		rmdirSync(path.dirname(file));
	} catch {
		// The folder still holds the entry's other picture.
	}
}

/**
 * Opens an entry in a skin and fails the test on a request off 127.0.0.1, a
 * page error, a watched construction, a play function that threw or
 * Storybook's error display, tagged `no-shot` or not; then, when it is shot,
 * compares it with its reference picture and checks again for anything since.
 */
export async function walkEntry(
	context: BrowserContext,
	page: Page,
	entry: Entry,
	skin: Skin
): Promise<void> {
	const watch = await watchContext(context, page);
	await page.goto(entryAddress(entry, skin));
	await expectRendered(page, entry.viewMode, skin);
	expectNoProblems(watch, 'before the picture');
	if (!isShot(entry)) return;
	const name = [entry.id, `${skin}.png`];
	// The project sets only `snapshotPathTemplate`, which the screenshot uses too.
	const picture = test.info().snapshotPath(...name);
	const before = bytesOf(picture);
	await expect(page).toHaveScreenshot(name);
	for (const failure of await playFailuresIn(page)) {
		watch.problems.add(`play function threw: ${failure}`);
	}
	// An update run never keeps a picture from a test that fails here.
	if (watch.problems.size > 0) putPictureBack(picture, before);
	expectNoProblems(watch, 'after the picture');
}
