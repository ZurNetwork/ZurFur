/**
 * What the screenshot walker reads from Storybook's build: the entries of
 * `index.json`, each id parsed before it names a picture, the address of an
 * entry in a skin, and the viewport its tags ask for.
 */
import { readdirSync, type Dirent } from 'node:fs';

/** The two skins every entry is shot in. */
export type Skin = 'dark' | 'light';

export const skins: readonly Skin[] = ['dark', 'light'];

/** How Storybook opens an entry: a story, or a docs page. */
export type ViewMode = 'story' | 'docs';

declare const entryIdBrand: unique symbol;

/** An entry id that may name a picture's folder: lower-case letters, digits and hyphens. */
export type EntryId = string & { readonly [entryIdBrand]: true };

/** True when `raw` may name a picture's folder. */
export function isEntryId(raw: string): raw is EntryId {
	return /^[a-z0-9-]+$/.test(raw);
}

/** The tag that leaves an entry out of the screenshots. */
export const noShotTag = 'no-shot';

/** The tag that shoots an entry at a phone's width. */
export const phoneTag = 'phone-320';

export interface Viewport {
	readonly width: number;
	readonly height: number;
}

/** The viewport every entry is shot at, unless it asks for the phone's. */
export const desktopViewport: Viewport = { width: 1280, height: 900 };

/** Storybook's `mobile1`, for an entry tagged `phone-320`. */
export const phoneViewport: Viewport = { width: 320, height: 568 };

export interface Entry {
	readonly id: EntryId;
	readonly viewMode: ViewMode;
	readonly tags: readonly string[];
}

/** One entry of the index: parsed, or refused with the reason (its test fails). */
export type IndexItem =
	| { readonly kind: 'entry'; readonly entry: Entry }
	| { readonly kind: 'refused'; readonly raw: string; readonly reason: string };

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

function isStringList(value: unknown): value is readonly string[] {
	return Array.isArray(value) && value.every((item) => typeof item === 'string');
}

function viewModeOf(type: unknown): ViewMode | undefined {
	return type === 'story' || type === 'docs' ? type : undefined;
}

function parseItem(key: string, value: unknown): IndexItem {
	if (!isFields(value)) return { kind: 'refused', raw: key, reason: 'the entry is not an object' };
	const { id, type, tags } = value;
	if (typeof id !== 'string') return { kind: 'refused', raw: key, reason: 'the id is not text' };
	if (id !== key) return { kind: 'refused', raw: id, reason: `the id is filed under ${key}` };
	if (!isEntryId(id)) {
		return {
			kind: 'refused',
			raw: id,
			reason: 'the id is not lower-case letters, digits and hyphens'
		};
	}
	const viewMode = viewModeOf(type);
	if (viewMode === undefined) {
		return {
			kind: 'refused',
			raw: id,
			reason: `the type ${JSON.stringify(type)} is neither story nor docs`
		};
	}
	const entryTags = tags ?? [];
	if (!isStringList(entryTags))
		return { kind: 'refused', raw: id, reason: 'the tags are not text' };
	return { kind: 'entry', entry: { id, viewMode, tags: entryTags } };
}

/** Reads `index.json`'s text into its items, in the index's order; throws when it is no index. */
export function parseIndex(text: string): readonly IndexItem[] {
	const index: unknown = JSON.parse(text);
	if (!isFields(index) || !isFields(index.entries)) {
		throw new Error('storybook-static/index.json holds no entries object');
	}
	return Object.entries(index.entries).map(([key, value]) => parseItem(key, value));
}

/** True when the entry is shot: it is not tagged `no-shot`. */
export function isShot(entry: Entry): boolean {
	return !entry.tags.includes(noShotTag);
}

/** The viewport an entry is shot at. */
export function viewportOf(entry: Entry): Viewport {
	return entry.tags.includes(phoneTag) ? phoneViewport : desktopViewport;
}

/** The preview address of an entry in a skin, with motion stopped. */
export function entryAddress(entry: Entry, skin: Skin): string {
	return `/iframe.html?id=${entry.id}&viewMode=${entry.viewMode}&globals=skin:${skin};motion:still`;
}

/** The picture file names, one per skin. */
const pictureNames: ReadonlySet<string> = new Set(skins.map((skin) => `${skin}.png`));

function entriesOf(folder: string): readonly Dirent[] {
	try {
		return readdirSync(folder, { withFileTypes: true });
	} catch (error) {
		if (error instanceof Error && 'code' in error && error.code === 'ENOENT') return [];
		throw error;
	}
}

/**
 * Every folder or file in the references folder that names no shot entry or
 * no skin, as a path relative to it. A missing folder holds nothing stale.
 */
export function stalePictures(referencesFolder: string, shot: readonly Entry[]): readonly string[] {
	const ids: ReadonlySet<string> = new Set(shot.map((entry) => entry.id));
	return entriesOf(referencesFolder).flatMap((folder) => {
		if (!folder.isDirectory() || !ids.has(folder.name)) return [folder.name];
		return entriesOf(`${referencesFolder}/${folder.name}`)
			.filter((file) => !file.isFile() || !pictureNames.has(file.name))
			.map((file) => `${folder.name}/${file.name}`);
	});
}
