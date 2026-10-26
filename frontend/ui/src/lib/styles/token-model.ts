import { parseSkin, type Skin } from './environment.ts';

declare const tokenNameBrand: unique symbol;

/** A token's name: lower case letters, digits and hyphens, starting with a letter. */
export type TokenName = string & { readonly [tokenNameBrand]: true };

const TOKEN_NAME = /^[a-z][a-z0-9-]*$/;
const WHOLE_REFERENCE = /^\{([a-z0-9-]+)\}$/;

/** True when the text is a well-formed token name. */
export function isTokenName(text: string): text is TokenName {
	return TOKEN_NAME.test(text);
}

// The allow-list: what a value may be once it reaches a stylesheet or a style
// attribute. Spaces are literal spaces, so a newline or a tab never passes.

const NUMBER = String.raw`-?(?:\d+|\d*\.\d+)(?:px|rem|em|%|ms|s|deg)?`;
const HEX = String.raw`#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})`;
const IDENT = String.raw`[a-z][a-z0-9-]*`;
const RGB = String.raw`rgba?\( *${NUMBER}(?: *, *${NUMBER}){2,3} *\)`;
const CUBIC_BEZIER = String.raw`cubic-bezier\( *${NUMBER}(?: *, *${NUMBER}){3} *\)`;
const ATOM = `(?:${NUMBER}|${HEX}|${IDENT}|${RGB}|${CUBIC_BEZIER})`;

/** A space- and comma-separated list of hex colours, numbers with a unit or none, idents, rgb()/rgba() and cubic-bezier(). */
const VALUE = new RegExp(`^${ATOM}(?: +${ATOM})*(?: *, *${ATOM}(?: +${ATOM})*)*$`, 'i');

const FAMILY_NAME = String.raw`(?:"[A-Za-z0-9 ]+"|[A-Za-z][A-Za-z0-9-]*)`;

/** A comma-separated list of quoted family names and idents. */
const FAMILY_LIST = new RegExp(`^${FAMILY_NAME}(?:, *${FAMILY_NAME})*$`);

/** True when a literal value may be written into a stylesheet or a style attribute. */
export function isAllowedValue(text: string): boolean {
	return VALUE.test(text);
}

/** True when a font family list may be written into a stylesheet. */
export function isAllowedFamilyList(text: string): boolean {
	return FAMILY_LIST.test(text);
}

/** Why the token files were turned away. */
export type ModelRefusalReason =
	| 'malformed'
	| 'bad-name'
	| 'value-off-allow-list'
	| 'duplicate-name'
	| 'embedded-reference'
	| 'dangling-reference'
	| 'cyclic-reference'
	| 'missing-skin'
	| 'unknown-theme'
	| 'bad-themes'
	| 'addition-exists';

/** One named reason a file was turned away, with where in the file and a plain sentence. */
export interface Refusal<Reason extends string = ModelRefusalReason> {
	readonly reason: Reason;
	readonly where: string;
	readonly detail: string;
}

/** One value as written: text to use as it stands, or a whole-value `{name}` reference. */
export type TokenValue =
	| { readonly kind: 'literal'; readonly text: string }
	| { readonly kind: 'reference'; readonly name: string };

/** A token's value: one for every skin, or one per skin. */
export type TokenValues =
	| { readonly kind: 'single'; readonly value: TokenValue }
	| { readonly kind: 'skinned'; readonly dark: TokenValue; readonly light: TokenValue };

/** A token's value with every reference followed, in each skin. */
export type SkinTexts = Readonly<Record<Skin, string>>;

/** The file a token came from. */
export type TokenSource = 'tokens' | 'additions';

export interface Token {
	readonly name: TokenName;
	readonly groupId: string;
	readonly values: TokenValues;
	/** The value in each skin with every `{name}` reference followed to its text. */
	readonly resolved: SkinTexts;
	readonly usage: string;
	readonly source: TokenSource;
}

export interface TokenGroup {
	readonly id: string;
	readonly note: string | undefined;
	readonly tokens: readonly Token[];
}

export interface Theme {
	readonly id: Skin;
	readonly name: string;
}

export interface FontFile {
	readonly family: string;
	readonly file: string;
	readonly weight: string;
	readonly style: string;
}

export interface TypeStyle {
	readonly name: string;
	readonly fontSize: string;
	readonly lineHeight: string;
	readonly fontWeight: number;
	readonly letterSpacing: string | undefined;
	readonly sample: string;
	readonly usage: string;
}

export interface TypeGroup {
	readonly name: string;
	/** The font family token's stem: `ui` names `--font-ui`. */
	readonly family: TokenName;
	readonly note: string;
	readonly styles: readonly TypeStyle[];
}

export interface TypeModel {
	readonly fonts: readonly FontFile[];
	readonly families: Readonly<{ ui: string; mono: string }>;
	readonly groups: readonly TypeGroup[];
}

export interface TokenModel {
	readonly name: string;
	readonly version: number;
	readonly themes: readonly Theme[];
	/** Every group except `sound`, in file order, additions merged into the group they name. */
	readonly groups: readonly TokenGroup[];
	readonly type: TypeModel;
	readonly sound: TokenGroup;
	/** Every token, `sound` included, by name. */
	readonly byName: ReadonlyMap<string, Token>;
}

/** What parsing the token files gives back: the model, or every reason it was turned away. */
export type ParseOutcome =
	| { readonly kind: 'parsed'; readonly model: TokenModel }
	| { readonly kind: 'refused'; readonly refusals: readonly Refusal[] };

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return typeof value === 'object' && Object(value) === value && !Array.isArray(value);
}

function isList(value: unknown): value is readonly unknown[] {
	return Array.isArray(value);
}

/** An own field of a parsed JSON object; inherited names such as `constructor` read as absent. */
function fieldOf(fields: Fields, key: string): unknown {
	return Object.hasOwn(fields, key) ? fields[key] : undefined;
}

/** The value of a token in one skin, before any reference is followed. */
export function valueIn(values: TokenValues, skin: Skin): TokenValue {
	switch (values.kind) {
		case 'single':
			return values.value;
		case 'skinned':
			return skin === 'dark' ? values.dark : values.light;
	}
}

/** The token with this name, `sound` included. */
export function findToken(model: TokenModel, name: string): Token | undefined {
	return model.byName.get(name);
}

/** The token's value in a skin, every reference followed; undefined when no token has the name. */
export function resolveToken(model: TokenModel, name: string, skin: Skin): string | undefined {
	return findToken(model, name)?.resolved[skin];
}

function refuse(reason: ModelRefusalReason, where: string, detail: string): Refusal {
	return { reason, where, detail };
}

/** A required text field of an object; a refusal is recorded when it is not text. */
function textField(
	fields: Fields,
	key: string,
	where: string,
	refusals: Refusal[]
): string | undefined {
	const value = fieldOf(fields, key);
	if (typeof value === 'string') return value;
	refusals.push(refuse('malformed', where, `"${key}" must be text`));
	return undefined;
}

function parseThemes(root: Fields, refusals: Refusal[]): readonly Theme[] | undefined {
	const color = fieldOf(root, 'color');
	const themes = isFields(color) ? fieldOf(color, 'themes') : undefined;
	if (!isList(themes)) {
		refusals.push(refuse('bad-themes', 'color.themes', 'the theme list is missing'));
		return undefined;
	}
	const parsed: Theme[] = [];
	for (const [index, entry] of themes.entries()) {
		const where = `color.themes[${String(index)}]`;
		const id = isFields(entry) ? textField(entry, 'id', where, refusals) : undefined;
		const name = isFields(entry) ? textField(entry, 'name', where, refusals) : undefined;
		const skin = parseSkin(id);
		if (id !== undefined && skin === undefined) {
			refusals.push(refuse('bad-themes', where, `"${id}" is not a skin this package styles`));
		}
		if (skin !== undefined && name !== undefined) {
			if (parsed.some((theme) => theme.id === skin)) {
				refusals.push(refuse('bad-themes', where, `"${skin}" is listed twice`));
			} else {
				parsed.push({ id: skin, name });
			}
		}
	}
	const skins = parsed.map((theme) => theme.id);
	const hasBoth = skins.includes('dark') && skins.includes('light');
	if (!hasBoth) {
		refusals.push(refuse('bad-themes', 'color.themes', 'the skins must be exactly dark and light'));
		return undefined;
	}
	return parsed;
}

function parseText(text: string, where: string, refusals: Refusal[]): TokenValue | undefined {
	const reference = WHOLE_REFERENCE.exec(text);
	const name = reference?.[1];
	if (name !== undefined) return { kind: 'reference', name };
	if (text.includes('{') || text.includes('}')) {
		refusals.push(
			refuse('embedded-reference', where, 'a {name} reference must be the whole value')
		);
		return undefined;
	}
	return { kind: 'literal', text };
}

function parseValues(
	raw: unknown,
	skins: readonly Skin[],
	where: string,
	refusals: Refusal[]
): TokenValues | undefined {
	if (typeof raw === 'string') {
		const value = parseText(raw, where, refusals);
		return value === undefined ? undefined : { kind: 'single', value };
	}
	if (!isFields(raw)) {
		refusals.push(refuse('malformed', where, '"value" must be text or one text per skin'));
		return undefined;
	}
	let isSound = true;
	for (const key of Object.keys(raw)) {
		const skin = parseSkin(key);
		if (skin === undefined || !skins.includes(skin)) {
			refusals.push(refuse('unknown-theme', where, `"${key}" is not one of the themes`));
			isSound = false;
		}
	}
	const perSkin = new Map<Skin, TokenValue>();
	for (const skin of skins) {
		const entry = fieldOf(raw, skin);
		if (entry === undefined) {
			refusals.push(refuse('missing-skin', where, `no value for "${skin}"`));
			isSound = false;
		} else if (typeof entry !== 'string') {
			refusals.push(refuse('malformed', `${where}.${skin}`, 'a value must be text'));
			isSound = false;
		} else {
			const value = parseText(entry, `${where}.${skin}`, refusals);
			if (value === undefined) isSound = false;
			else perSkin.set(skin, value);
		}
	}
	const dark = perSkin.get('dark');
	const light = perSkin.get('light');
	if (!isSound || dark === undefined || light === undefined) return undefined;
	return { kind: 'skinned', dark, light };
}

interface Draft {
	readonly name: TokenName;
	readonly groupId: string;
	readonly values: TokenValues;
	readonly usage: string;
	readonly source: TokenSource;
}

interface DraftGroup {
	readonly id: string;
	readonly note: string | undefined;
	readonly drafts: Draft[];
}

function parseGroup(
	raw: unknown,
	id: string,
	source: TokenSource,
	skins: readonly Skin[],
	refusals: Refusal[]
): DraftGroup | undefined {
	const where = `${source}:${id}`;
	const tokens = isFields(raw) ? fieldOf(raw, 'tokens') : undefined;
	if (!isFields(raw) || !isList(tokens)) {
		refusals.push(refuse('malformed', where, 'a group is an object with a "tokens" list'));
		return undefined;
	}
	const note = fieldOf(raw, 'note');
	const drafts: Draft[] = [];
	for (const [index, entry] of tokens.entries()) {
		const at = `${where}[${String(index)}]`;
		if (!isFields(entry)) {
			refusals.push(refuse('malformed', at, 'a token is an object'));
			continue;
		}
		const name = textField(entry, 'name', at, refusals);
		const usage = textField(entry, 'usage', at, refusals);
		if (name !== undefined && !isTokenName(name)) {
			refusals.push(refuse('bad-name', at, `"${name}" is not a token name`));
			continue;
		}
		if (name === undefined || usage === undefined) continue;
		const values = parseValues(fieldOf(entry, 'value'), skins, name, refusals);
		if (values === undefined) continue;
		drafts.push({ name, groupId: id, values, usage, source });
	}
	return { id, note: typeof note === 'string' ? note : undefined, drafts };
}

/** Follows references from one value to a literal; undefined (with a refusal) on a loop or a gap. */
function resolveText(
	start: TokenValue,
	owner: TokenName,
	skin: Skin,
	values: ReadonlyMap<string, TokenValues>,
	refusals: Refusal[]
): string | undefined {
	const seen = new Set<string>([owner]);
	let current = start;
	for (;;) {
		switch (current.kind) {
			case 'literal':
				return current.text;
			case 'reference': {
				const where = `${owner}.${skin}`;
				if (seen.has(current.name)) {
					refusals.push(
						refuse('cyclic-reference', where, `{${current.name}} leads back to itself`)
					);
					return undefined;
				}
				seen.add(current.name);
				const target = values.get(current.name);
				if (target === undefined) {
					refusals.push(refuse('dangling-reference', where, `no token named ${current.name}`));
					return undefined;
				}
				current = valueIn(target, skin);
			}
		}
	}
}

function parseFonts(raw: unknown, refusals: Refusal[]): readonly FontFile[] | undefined {
	if (!isList(raw)) {
		refusals.push(refuse('malformed', 'type.fonts', 'the font list is missing'));
		return undefined;
	}
	const fonts: FontFile[] = [];
	for (const [index, entry] of raw.entries()) {
		const where = `type.fonts[${String(index)}]`;
		if (!isFields(entry)) {
			refusals.push(refuse('malformed', where, 'a font is an object'));
			continue;
		}
		const family = textField(entry, 'family', where, refusals);
		const file = textField(entry, 'file', where, refusals);
		const weight = textField(entry, 'weight', where, refusals);
		const style = textField(entry, 'style', where, refusals);
		if (family !== undefined && file !== undefined && weight !== undefined && style !== undefined) {
			fonts.push({ family, file, weight, style });
		}
	}
	return fonts;
}

/**
 * A style field's text when it is on the allow-list; otherwise a refusal naming
 * the field is recorded. The refusal leaves the text out, since pages show it.
 */
function allowedValue(
	text: string | undefined,
	field: string,
	where: string,
	refusals: Refusal[]
): string | undefined {
	if (text === undefined || isAllowedValue(text)) return text;
	refusals.push(
		refuse('value-off-allow-list', `${where}.${field}`, `"${field}" is not on the allow-list`)
	);
	return undefined;
}

function parseTypeStyle(raw: unknown, where: string, refusals: Refusal[]): TypeStyle | undefined {
	if (!isFields(raw)) {
		refusals.push(refuse('malformed', where, 'a type style is an object'));
		return undefined;
	}
	const name = textField(raw, 'name', where, refusals);
	const rawFontSize = textField(raw, 'fontSize', where, refusals);
	const rawLineHeight = textField(raw, 'lineHeight', where, refusals);
	const sample = textField(raw, 'sample', where, refusals);
	const usage = textField(raw, 'usage', where, refusals);
	const fontWeight = fieldOf(raw, 'fontWeight');
	const rawLetterSpacing = fieldOf(raw, 'letterSpacing');
	if (typeof fontWeight !== 'number') {
		refusals.push(refuse('malformed', where, '"fontWeight" must be a number'));
	}
	const isLetterSpacingText =
		rawLetterSpacing === undefined || typeof rawLetterSpacing === 'string';
	if (!isLetterSpacingText) {
		refusals.push(refuse('malformed', where, '"letterSpacing" must be text when present'));
	}
	const fontSize = allowedValue(rawFontSize, 'fontSize', where, refusals);
	const lineHeight = allowedValue(rawLineHeight, 'lineHeight', where, refusals);
	const letterSpacing = allowedValue(
		typeof rawLetterSpacing === 'string' ? rawLetterSpacing : undefined,
		'letterSpacing',
		where,
		refusals
	);
	const isLetterSpacingSound = rawLetterSpacing === undefined || letterSpacing !== undefined;
	const isWhole =
		name !== undefined &&
		fontSize !== undefined &&
		lineHeight !== undefined &&
		sample !== undefined &&
		usage !== undefined &&
		typeof fontWeight === 'number' &&
		isLetterSpacingSound;
	if (!isWhole) return undefined;
	return { name, fontSize, lineHeight, fontWeight, letterSpacing, sample, usage };
}

function parseTypeGroup(raw: unknown, where: string, refusals: Refusal[]): TypeGroup | undefined {
	if (!isFields(raw)) {
		refusals.push(refuse('malformed', where, 'a type group is an object'));
		return undefined;
	}
	const name = textField(raw, 'name', where, refusals);
	const family = textField(raw, 'family', where, refusals);
	const note = textField(raw, 'note', where, refusals);
	const isFamilyName = family !== undefined && isTokenName(family);
	if (family !== undefined && !isFamilyName) {
		refusals.push(
			refuse('bad-name', `${where}.family`, '"family" must be a token name, such as ui')
		);
	}
	const rawStyles = fieldOf(raw, 'styles');
	if (!isList(rawStyles)) {
		refusals.push(refuse('malformed', where, '"styles" must be a list'));
		return undefined;
	}
	const styles = rawStyles.flatMap((entry, index) => {
		const style = parseTypeStyle(entry, `${where}.styles[${String(index)}]`, refusals);
		return style === undefined ? [] : [style];
	});
	if (name === undefined || !isFamilyName || note === undefined) return undefined;
	return { name, family, note, styles };
}

function parseType(raw: unknown, refusals: Refusal[]): TypeModel | undefined {
	if (!isFields(raw)) {
		refusals.push(refuse('malformed', 'type', 'the type section is missing'));
		return undefined;
	}
	const fonts = parseFonts(fieldOf(raw, 'fonts'), refusals);
	const rawFamilies = fieldOf(raw, 'families');
	const ui = isFields(rawFamilies)
		? textField(rawFamilies, 'ui', 'type.families', refusals)
		: undefined;
	const mono = isFields(rawFamilies)
		? textField(rawFamilies, 'mono', 'type.families', refusals)
		: undefined;
	if (!isFields(rawFamilies)) {
		refusals.push(refuse('malformed', 'type.families', 'the font families are missing'));
	}
	const rawGroups = fieldOf(raw, 'groups');
	if (!isList(rawGroups)) {
		refusals.push(refuse('malformed', 'type.groups', 'the type groups are missing'));
		return undefined;
	}
	const groups = rawGroups.flatMap((entry, index) => {
		const group = parseTypeGroup(entry, `type.groups[${String(index)}]`, refusals);
		return group === undefined ? [] : [group];
	});
	if (fonts === undefined || ui === undefined || mono === undefined) return undefined;
	return { fonts, families: { ui, mono }, groups };
}

/** The keys of the token file that are not token groups. */
const NOT_GROUPS: ReadonlySet<string> = new Set(['name', 'version', 'type']);

/** Reads the groups of one file, in file order. */
function parseGroups(
	root: Fields,
	source: TokenSource,
	skins: readonly Skin[],
	refusals: Refusal[]
): readonly DraftGroup[] {
	const groups: DraftGroup[] = [];
	for (const [id, raw] of Object.entries(root)) {
		if (source === 'tokens' && NOT_GROUPS.has(id)) continue;
		const group = parseGroup(raw, id, source, skins, refusals);
		if (group !== undefined) groups.push(group);
	}
	return groups;
}

/** Folds the additions into the groups they name (a group only they name is added last). */
function mergeAdditions(
	base: readonly DraftGroup[],
	additions: readonly DraftGroup[],
	refusals: Refusal[]
): readonly DraftGroup[] {
	const merged = base.map((group) => ({ ...group, drafts: [...group.drafts] }));
	const taken = new Set(merged.flatMap((group) => group.drafts.map((draft) => draft.name)));
	for (const addition of additions) {
		let target = merged.find((group) => group.id === addition.id);
		if (target === undefined) {
			target = { id: addition.id, note: addition.note, drafts: [] };
			merged.push(target);
		}
		for (const draft of addition.drafts) {
			if (taken.has(draft.name)) {
				refusals.push(
					refuse(
						'addition-exists',
						draft.name,
						'the token file already has this name; drop the addition'
					)
				);
				continue;
			}
			taken.add(draft.name);
			target.drafts.push(draft);
		}
	}
	return merged;
}

/** Resolves every draft in every skin; refusals are recorded for loops and gaps. */
function resolveGroups(
	groups: readonly DraftGroup[],
	skins: readonly Skin[],
	refusals: Refusal[]
): readonly TokenGroup[] {
	const values = new Map<string, TokenValues>();
	for (const group of groups) {
		for (const draft of group.drafts) {
			if (values.has(draft.name)) {
				refusals.push(refuse('duplicate-name', draft.name, 'two tokens share this name'));
			} else {
				values.set(draft.name, draft.values);
			}
		}
	}
	const resolveIn = (draft: Draft, skin: Skin): string =>
		resolveText(valueIn(draft.values, skin), draft.name, skin, values, refusals) ?? '';
	return groups.map((group) => ({
		id: group.id,
		note: group.note,
		tokens: group.drafts.map((draft) => ({
			name: draft.name,
			groupId: draft.groupId,
			values: draft.values,
			resolved: { dark: resolveIn(draft, 'dark'), light: resolveIn(draft, 'light') },
			usage: draft.usage,
			source: draft.source
		}))
	}));
}

/**
 * The one parser of the token file's shape. Takes the parsed `tokens.json` and
 * the parsed `tokens.additions.json` and returns a typed, readonly model with
 * every reference followed in each skin, or every reason the files were turned
 * away. It never throws.
 */
export function parseTokenModel(tokens: unknown, additions: unknown): ParseOutcome {
	const refusals: Refusal[] = [];
	if (!isFields(tokens)) {
		return {
			kind: 'refused',
			refusals: [refuse('malformed', 'tokens', 'the token file must be an object')]
		};
	}
	if (!isFields(additions)) {
		return {
			kind: 'refused',
			refusals: [refuse('malformed', 'additions', 'the additions file must be an object')]
		};
	}
	const themes = parseThemes(tokens, refusals);
	if (themes === undefined) return { kind: 'refused', refusals };
	const skins = themes.map((theme) => theme.id);
	const name = fieldOf(tokens, 'name');
	const version = fieldOf(tokens, 'version');
	if (typeof name !== 'string') refusals.push(refuse('malformed', 'name', '"name" must be text'));
	if (typeof version !== 'number') {
		refusals.push(refuse('malformed', 'version', '"version" must be a number'));
	}
	const type = parseType(fieldOf(tokens, 'type'), refusals);
	const own = parseGroups(tokens, 'tokens', skins, refusals);
	const added = parseGroups(additions, 'additions', skins, refusals);
	const resolved = resolveGroups(mergeAdditions(own, added, refusals), skins, refusals);
	const sound = resolved.find((group) => group.id === 'sound');
	if (sound === undefined)
		refusals.push(refuse('malformed', 'sound', 'the sound group is missing'));
	const isWhole =
		typeof name === 'string' &&
		typeof version === 'number' &&
		type !== undefined &&
		sound !== undefined;
	if (refusals.length > 0 || !isWhole) return { kind: 'refused', refusals };
	const groups = resolved.filter((group) => group.id !== 'sound');
	const byName = new Map(
		resolved.flatMap((group) => group.tokens.map((token): [string, Token] => [token.name, token]))
	);
	return { kind: 'parsed', model: { name, version, themes, groups, type, sound, byName } };
}
