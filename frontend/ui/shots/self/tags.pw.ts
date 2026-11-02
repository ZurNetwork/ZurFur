/**
 * The tags' viewports, proved: an entry tagged `phone-320` is shot at a
 * phone's size, every other entry at the desktop's.
 */
import { expect, test } from '@playwright/test';
import { isEntryId, phoneTag, viewportOf, type Entry } from '../storybook.ts';

function entryTagged(tags: readonly string[]): Entry {
	const id = 'controls-button--primary';
	if (!isEntryId(id)) throw new Error(`${id} is not an entry id`);
	return { id, viewMode: 'story', tags };
}

test('an entry tagged phone-320 is shot at 320×568, any other at 1280×900', () => {
	expect(viewportOf(entryTagged([phoneTag]))).toEqual({ width: 320, height: 568 });
	expect(viewportOf(entryTagged(['autodocs', 'dev']))).toEqual({ width: 1280, height: 900 });
});
