/**
 * The walker's checks, proved: each catches what it is there to catch, with
 * the same helpers the walker uses.
 */
import { expect, test } from '@playwright/test';
import { imageGuardRefusals } from '../image-guard.ts';
import {
	playFailuresIn,
	shotsOrigin,
	waitForStorybook,
	watchContext,
	watchedConstructors
} from '../watch.ts';

test('a missing reference picture fails the test', async ({ page }) => {
	test.fail();
	await page.setContent('<p>No reference picture is ever made for this page.</p>');
	await expect(page).toHaveScreenshot(['missing', 'dark.png']);
});

test('a request off 127.0.0.1 is caught, and one to the server is not', async ({
	context,
	page
}) => {
	const watch = await watchContext(context, page);
	const offHost = page.waitForEvent('requestfailed', (request) =>
		request.url().startsWith('https://example.org/')
	);
	await page.setContent(
		`<img alt="" src="${shotsOrigin}/favicon.svg"><img alt="" src="https://example.org/x.png">`
	);
	await offHost;
	expect([...watch.problems]).toEqual(['request to https://example.org/x.png']);
});

test('a request off 127.0.0.1 from a nested frame is caught', async ({ context, page }) => {
	const watch = await watchContext(context, page);
	const offHost = page.waitForEvent('requestfailed', (request) =>
		request.url().startsWith('https://example.org/')
	);
	await page.setContent(
		`<iframe title="nested" srcdoc='<img alt="" src="https://example.org/nested.png">'></iframe>`
	);
	await offHost;
	expect([...watch.problems]).toEqual(['request to https://example.org/nested.png']);
});

test('a websocket off 127.0.0.1 is caught', async ({ context, page }) => {
	const watch = await watchContext(context, page);
	await page.evaluate(() => {
		new WebSocket('wss://example.org/socket');
	});
	await expect.poll(() => [...watch.problems]).toContain('websocket to wss://example.org/socket');
});

test('a page error is caught, in the page and in a nested frame', async ({ context, page }) => {
	const watch = await watchContext(context, page);
	await page.setContent(
		[
			'<script>throw new Error("thrown by the page")</script>',
			`<iframe title="nested" srcdoc='<script>throw new Error("thrown by a frame")</script>'></iframe>`
		].join('')
	);
	await expect
		.poll(() => [...watch.problems].sort())
		.toEqual(['page error: Error: thrown by a frame', 'page error: Error: thrown by the page']);
});

test("Storybook's error display is caught", async ({ context, page }) => {
	const watch = await watchContext(context, page);
	await page.goto('/iframe.html?id=no-such-entry&viewMode=story&globals=skin:dark;motion:still');
	expect(await waitForStorybook(page, 'story')).toBe('error-display');
	expect([...watch.problems]).toEqual([]);
});

/**
 * Plants a stand-in for Storybook's preview, every story render finished, and
 * its channel; when `playThrows`, the channel then reports a play function
 * that threw, as Storybook does before it goes on to finish.
 */
function plantFinishedStory(playThrows: boolean): void {
	const listeners = new Map<string, (payload: unknown) => void>();
	Reflect.set(globalThis, '__STORYBOOK_PREVIEW__', { storyRenders: [{ phase: 'finished' }] });
	Reflect.set(globalThis, '__STORYBOOK_ADDONS_CHANNEL__', {
		on: (name: string, listener: (payload: unknown) => void) => {
			listeners.set(name, listener);
		}
	});
	const thrown = { name: 'AssertionError', message: 'planted by the self-test\nits second line' };
	if (playThrows) listeners.get('playFunctionThrewException')?.(thrown);
}

test('a play function that throws is caught and named, and a finished story without one is not', async ({
	context,
	page
}) => {
	await watchContext(context, page);
	await page.goto('/index.json');
	await page.evaluate(plantFinishedStory, false);
	expect(await waitForStorybook(page, 'story')).toBe('rendered');
	await page.goto('/index.json');
	await page.evaluate(plantFinishedStory, true);
	expect(await waitForStorybook(page, 'story')).toBe('play-failed');
	expect(await playFailuresIn(page)).toEqual(['AssertionError: planted by the self-test']);
});

/** A served page, so the constructors that need a secure context have one. */
const servedPage = '/iframe.html?id=no-such-entry&viewMode=story&globals=skin:dark;motion:still';

/** Builds each watched constructor the page has and says which it built. */
function buildEach(names: readonly string[]): string[] {
	const built: string[] = [];
	for (const name of names) {
		const constructor: unknown = Reflect.get(globalThis, name);
		if (typeof constructor !== 'function') continue;
		const argument =
			name === 'SharedWorker'
				? URL.createObjectURL(new Blob([''], { type: 'text/javascript' }))
				: name === 'WebTransport'
					? 'https://example.org/transport'
					: undefined;
		const instance: unknown = Reflect.construct(
			constructor,
			argument === undefined ? [] : [argument]
		);
		for (const settled of ['ready', 'closed']) {
			const promise: unknown =
				Object(instance) === instance ? Reflect.get(Object(instance), settled) : undefined;
			if (promise instanceof Promise) promise.catch(() => undefined);
		}
		built.push(name);
	}
	return built;
}

test('a SharedWorker, RTCPeerConnection or WebTransport built by the page is caught', async ({
	context,
	page
}) => {
	const watch = await watchContext(context, page);
	await page.goto(servedPage);
	const built = await page.evaluate(buildEach, watchedConstructors);
	expect(built).toEqual(
		expect.arrayContaining(['SharedWorker', 'RTCPeerConnection', 'WebTransport'])
	);
	await expect
		.poll(() => [...watch.problems].sort())
		.toEqual(built.map((name) => `construction of ${name}`).sort());
});

test('a SharedWorker built by a nested frame is caught', async ({ context, page }) => {
	const watch = await watchContext(context, page);
	await page.goto(servedPage);
	await page.evaluate(() => {
		const frame = document.createElement('iframe');
		frame.title = 'nested';
		frame.srcdoc = `<script>new SharedWorker(URL.createObjectURL(new Blob([''], { type: 'text/javascript' })))</script>`;
		document.body.append(frame);
	});
	await expect.poll(() => [...watch.problems]).toEqual(['construction of SharedWorker']);
});

test('the guard refuses outside the image', () => {
	const pin =
		'mcr.microsoft.com/playwright:v1.61.1-noble@sha256:cf0daee9b994042e011bc29f20cdff1a9f682a039b43fcd738f7d8a9d3bcd9d6';
	expect(imageGuardRefusals({ pin, marker: undefined, installed: '1.61.1' })).toEqual([
		'the image marker /ms-playwright/.docker-info is missing: not the Playwright image'
	]);
});
