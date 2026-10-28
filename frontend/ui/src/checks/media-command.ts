/**
 * The `emulateMedia` browser command. It runs in Vitest's Node process, with
 * the whole page that holds the test frame in reach, so it reads its payload
 * as untrusted: only `colorScheme` (`light`, `dark` or `null`),
 * `reducedMotion` (`reduce`, `no-preference` or `null`) and `forcedColors`
 * (`active`, `none` or `null`), as own data properties of a plain object,
 * reach the page; anything else is refused before the page is touched.
 */
import type { BrowserCommand } from 'vitest/node';

/** The three media features a spec may emulate; `null` hands one back to the browser. */
export interface MediaSettings {
	readonly colorScheme?: ColorScheme;
	readonly reducedMotion?: ReducedMotion;
	readonly forcedColors?: ForcedColors;
}

type ColorScheme = 'light' | 'dark' | null;
type ReducedMotion = 'reduce' | 'no-preference' | null;
type ForcedColors = 'active' | 'none' | null;

/** What the command did: applied the settings, or refused them and touched nothing. */
export type MediaOutcome =
	{ readonly kind: 'applied' } | { readonly kind: 'refused'; readonly reason: string };

type Parsed =
	| { readonly kind: 'parsed'; readonly settings: MediaSettings }
	| { readonly kind: 'refused'; readonly reason: string };

const colorSchemes: readonly unknown[] = ['light', 'dark', null];
const reducedMotions: readonly unknown[] = ['reduce', 'no-preference', null];
const forcedColorsValues: readonly unknown[] = ['active', 'none', null];

function isColorScheme(value: unknown): value is ColorScheme {
	return colorSchemes.includes(value);
}

function isReducedMotion(value: unknown): value is ReducedMotion {
	return reducedMotions.includes(value);
}

function isForcedColors(value: unknown): value is ForcedColors {
	return forcedColorsValues.includes(value);
}

function refused(reason: string): Parsed {
	return { kind: 'refused', reason };
}

/** The value of an own data property, never running a getter; undefined when there is none. */
function ownValue(object: object, key: string): { readonly value: unknown } | undefined {
	const descriptor = Object.getOwnPropertyDescriptor(object, key);
	return descriptor !== undefined && 'value' in descriptor
		? { value: descriptor.value }
		: undefined;
}

/** Reads a payload into settings, or refuses it. */
export function parseMediaSettings(payload: unknown): Parsed {
	if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) {
		return refused('the settings are not an object');
	}
	const prototype: unknown = Object.getPrototypeOf(payload);
	if (prototype !== Object.prototype && prototype !== null) {
		return refused('the settings are not a plain object');
	}
	const keys = Reflect.ownKeys(payload);
	const unknownKeys = keys.filter(
		(key) => key !== 'colorScheme' && key !== 'reducedMotion' && key !== 'forcedColors'
	);
	if (unknownKeys.length > 0) {
		return refused(`unknown setting(s): ${unknownKeys.map(String).join(', ')}`);
	}
	const settings: {
		colorScheme?: ColorScheme;
		reducedMotion?: ReducedMotion;
		forcedColors?: ForcedColors;
	} = {};
	if (keys.includes('colorScheme')) {
		const colorScheme = ownValue(payload, 'colorScheme');
		if (colorScheme === undefined || !isColorScheme(colorScheme.value)) {
			return refused('colorScheme is not light, dark or null');
		}
		settings.colorScheme = colorScheme.value;
	}
	if (keys.includes('reducedMotion')) {
		const reducedMotion = ownValue(payload, 'reducedMotion');
		if (reducedMotion === undefined || !isReducedMotion(reducedMotion.value)) {
			return refused('reducedMotion is not reduce, no-preference or null');
		}
		settings.reducedMotion = reducedMotion.value;
	}
	if (keys.includes('forcedColors')) {
		const forcedColors = ownValue(payload, 'forcedColors');
		if (forcedColors === undefined || !isForcedColors(forcedColors.value)) {
			return refused('forcedColors is not active, none or null');
		}
		settings.forcedColors = forcedColors.value;
	}
	return { kind: 'parsed', settings };
}

/** Emulates the parsed settings on the page that holds the test frame; nothing else reaches it. */
export const emulateMedia: BrowserCommand<[unknown], MediaOutcome> = async (context, payload) => {
	const parsed = parseMediaSettings(payload);
	if (parsed.kind === 'refused') return parsed;
	await context.page.emulateMedia(parsed.settings);
	return { kind: 'applied' };
};

declare module 'vitest/browser' {
	interface BrowserCommands {
		emulateMedia: (settings: MediaSettings) => Promise<MediaOutcome>;
	}
}
