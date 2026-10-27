/**
 * The walker's checks, proved: each catches what it is there to catch, with
 * the same helpers the walker uses.
 */
import { expect, test } from '@playwright/test';
import { imageGuardRefusals } from '../image-guard.ts';
import { shotsOrigin, waitForStorybook, watchContext } from '../watch.ts';

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

test('the guard refuses outside the image', () => {
	const pin =
		'mcr.microsoft.com/playwright:v1.61.1-noble@sha256:cf0daee9b994042e011bc29f20cdff1a9f682a039b43fcd738f7d8a9d3bcd9d6';
	expect(imageGuardRefusals({ pin, marker: undefined, installed: '1.61.1' })).toEqual([
		'the image marker /ms-playwright/.docker-info is missing: not the Playwright image'
	]);
});
