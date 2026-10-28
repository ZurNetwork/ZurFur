/**
 * The walk of one entry, proved: an entry tagged `no-shot` is still opened and
 * checked, and only its picture is left out; a shot entry is checked again
 * after its picture, and a picture from a test that fails there is put back.
 */
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { expect, test } from '@playwright/test';
import { entryAddress, noShotTag, parseIndex, type Entry, type IndexItem } from '../storybook.ts';
import { expectRendered, putPictureBack, walkEntry } from '../watch.ts';

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

/** Plants an input that builds a SharedWorker once a picture hides its caret. */
function plantConstructionAtPicture(): void {
	document.addEventListener('DOMContentLoaded', () => {
		const input = document.createElement('input');
		document.body.append(input);
		const observer = new MutationObserver(() => {
			if (input.style.getPropertyValue('caret-color') !== 'transparent') return;
			const code = new Blob([''], { type: 'text/javascript' });
			Reflect.construct(SharedWorker, [URL.createObjectURL(code)]);
		});
		observer.observe(input, { attributeFilter: ['style'] });
	});
}

test('a construction while the picture is taken fails the check after it', async ({
	context,
	page
}) => {
	// Light, so the missing-picture test above still finds no dark picture.
	const entry = firstStory();
	await context.addInitScript(plantConstructionAtPicture);
	await page.goto(entryAddress(entry, 'light'));
	await expectRendered(page, entry.viewMode, 'light');
	const picture = test.info().snapshotPath(entry.id, 'light.png');
	await page.screenshot({ path: picture, animations: 'disabled', caret: 'hide', scale: 'css' });
	await expect(walkEntry(context, page, entry, 'light')).rejects.toThrow(
		/after the picture[\s\S]*construction of SharedWorker/
	);
});

test('a picture from a failed test is put back as it was', () => {
	const folder = test.info().outputPath('references');
	const written = path.join(folder, 'new-entry', 'dark.png');
	mkdirSync(path.dirname(written), { recursive: true });
	writeFileSync(written, 'written by the run');
	putPictureBack(written, undefined);
	expect(existsSync(path.dirname(written)), 'a new picture and its folder are gone').toBe(false);

	const beside = path.join(folder, 'two-pictures', 'dark.png');
	const sibling = path.join(folder, 'two-pictures', 'light.png');
	mkdirSync(path.dirname(beside), { recursive: true });
	writeFileSync(beside, 'written by the run');
	writeFileSync(sibling, 'kept');
	putPictureBack(beside, undefined);
	expect(existsSync(beside), 'the new picture is gone').toBe(false);
	expect(readFileSync(sibling, 'utf8'), 'its sibling stays').toBe('kept');

	const changed = path.join(folder, 'changed', 'dark.png');
	mkdirSync(path.dirname(changed), { recursive: true });
	writeFileSync(changed, 'written by the run');
	putPictureBack(changed, Buffer.from('the old picture'));
	expect(readFileSync(changed, 'utf8'), 'a changed picture gets its old bytes').toBe(
		'the old picture'
	);

	const unchanged = path.join(folder, 'unchanged', 'dark.png');
	mkdirSync(path.dirname(unchanged), { recursive: true });
	writeFileSync(unchanged, 'the same picture');
	chmodSync(unchanged, 0o444);
	putPictureBack(unchanged, Buffer.from('the same picture'));
	expect(readFileSync(unchanged, 'utf8'), 'an unchanged picture is never written').toBe(
		'the same picture'
	);
});
