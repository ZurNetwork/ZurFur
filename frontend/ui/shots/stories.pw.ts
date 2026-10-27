/**
 * The screenshot walker: every story and docs entry of the built Storybook,
 * in Paper dark and Paper light with motion stopped, checked for requests off
 * 127.0.0.1, page errors, SharedWorker, RTCPeerConnection and WebTransport
 * constructions and Storybook's error display, then compared with its
 * reference picture unless it is tagged `no-shot`, and checked again after
 * it. Also: the manager page under the same checks, and no reference picture
 * left behind by a renamed or removed entry.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@playwright/test';
import { isShot, parseIndex, skins, stalePictures, viewportOf, type Entry } from './storybook.ts';
import { expectNoProblems, settle, walkEntry, watchContext, waitForStorybook } from './watch.ts';

const shotsFolder = import.meta.dirname;
const indexFile = path.join(shotsFolder, '..', 'storybook-static', 'index.json');
const referencesFolder = path.join(shotsFolder, '__screenshots__');

const items = parseIndex(readFileSync(indexFile, 'utf8'));
const entries: readonly Entry[] = items.flatMap((item) =>
	item.kind === 'entry' ? [item.entry] : []
);

for (const item of items) {
	if (item.kind === 'refused') {
		test(`index entry ${JSON.stringify(item.raw)}`, () => {
			throw new Error(`${JSON.stringify(item.raw)}: ${item.reason}`);
		});
	}
}

for (const entry of entries) {
	test.describe(entry.id, () => {
		test.use({ viewport: viewportOf(entry) });

		for (const skin of skins) {
			test.describe(skin, () => {
				test.use({ colorScheme: skin });

				test(`${entry.viewMode} ${entry.id} in ${skin}`, async ({ context, page }) => {
					await walkEntry(context, page, entry, skin);
				});
			});
		}
	});
}

test('the manager page loads nothing off 127.0.0.1', async ({ context, page }) => {
	const watch = await watchContext(context, page);
	await page.goto('/index.html');
	const preview = await page.locator('#storybook-preview-iframe').elementHandle();
	const frame = (await preview?.contentFrame()) ?? undefined;
	if (frame === undefined) throw new Error("the manager's preview frame is missing");
	await frame.waitForURL(/\/iframe\.html\?/);
	const viewMode = new URL(frame.url()).searchParams.get('viewMode') === 'docs' ? 'docs' : 'story';
	expect(await waitForStorybook(frame, viewMode), "the manager's preview state").toBe('rendered');
	await settle(page);
	expectNoProblems(watch, 'on the manager page');
});

test('no reference picture names an entry or skin the walker does not shoot', () => {
	const shot = entries.filter(isShot);
	expect(stalePictures(referencesFolder, shot), 'stale pictures under __screenshots__').toEqual([]);
});
