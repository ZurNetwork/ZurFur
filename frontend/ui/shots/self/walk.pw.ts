/**
 * The walk of one entry, proved: an entry tagged `no-shot` is still opened and
 * checked, and only its picture is left out.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@playwright/test';
import { noShotTag, parseIndex, walkEntry, type Entry, type IndexItem } from '../storybook.ts';

const indexFile = path.join(import.meta.dirname, '..', '..', 'storybook-static', 'index.json');

function entriesOf(items: readonly IndexItem[]): readonly Entry[] {
	return items.flatMap((item) => (item.kind === 'entry' ? [item.entry] : []));
}

/** The built index's first story without the `no-shot` tag. */
function firstStory(): Entry {
	const builtIndex = parseIndex(readFileSync(indexFile, 'utf8'));
	const story = entriesOf(builtIndex).find(
		(entry) => entry.viewMode === 'story' && !entry.tags.includes(noShotTag)
	);
	if (story === undefined) throw new Error('the index holds no story without the no-shot tag');
	return story;
}

/** The built index's first story, tagged `no-shot`. */
function plantedNoShot(): Entry {
	const story = firstStory();
	return { ...story, tags: [...story.tags, noShotTag] };
}

test('a shot entry is compared with its picture', async ({ context, page }) => {
	await expect(walkEntry(context, page, firstStory(), 'dark')).rejects.toThrow(
		"A snapshot doesn't exist"
	);
});

test('a no-shot entry is opened and checked, with no picture taken', async ({ context, page }) => {
	const entry = plantedNoShot();
	await walkEntry(context, page, entry, 'dark');
	expect(page.url()).toContain(`id=${entry.id}&`);
});

test('a request off 127.0.0.1 from a no-shot entry fails it', async ({ context, page }) => {
	await context.addInitScript(() => {
		void fetch('https://example.org/planted.json').catch(() => undefined);
	});
	await expect(walkEntry(context, page, plantedNoShot(), 'dark')).rejects.toThrow(
		'request to https://example.org/planted.json'
	);
});

test('a page error in a no-shot entry fails it', async ({ context, page }) => {
	await context.addInitScript(() => {
		setTimeout(() => {
			throw new Error('planted in a no-shot entry');
		});
	});
	await expect(walkEntry(context, page, plantedNoShot(), 'dark')).rejects.toThrow(
		'page error: Error: planted in a no-shot entry'
	);
});

test("Storybook's error display on a no-shot entry fails it", async ({ context, page }) => {
	const plantedIndex = JSON.stringify({
		entries: { 'no-such-entry': { id: 'no-such-entry', type: 'story', tags: [noShotTag] } }
	});
	const [missing] = entriesOf(parseIndex(plantedIndex));
	if (missing === undefined) throw new Error('the planted index holds no entry');
	await expect(walkEntry(context, page, missing, 'dark')).rejects.toThrow(
		"Storybook's preview state"
	);
});
