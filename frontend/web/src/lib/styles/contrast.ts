/**
 * Contrast arithmetic over the colours `tokens.css` declares: read the dark
 * `:root` block, follow `var()` chains to a colour, parse hex and `rgb()`
 * values, composite a translucent colour over the one beneath it, and compute
 * relative luminance and contrast ratio as WCAG 2.2 defines them.
 */

/** An sRGB colour: channels 0–255 (unrounded once composited), alpha 0–1. */
export interface Rgba {
	readonly red: number;
	readonly green: number;
	readonly blue: number;
	readonly alpha: number;
}

/** A value read from the stylesheet, or why it couldn't be. */
export type Read<T> =
	| { readonly kind: 'read'; readonly value: T }
	| { readonly kind: 'unreadable'; readonly reason: string };

function read<T>(value: T): Read<T> {
	return { kind: 'read', value };
}

function unreadable<T>(reason: string): Read<T> {
	return { kind: 'unreadable', reason };
}

const COMMENT = /\/\*[\s\S]*?\*\//g;

const CUSTOM_PROPERTY = /(--[\w-]+)\s*:\s*([^;]+);/g;

/** The body of the first top-level rule whose selector is exactly `:root`: no attribute, outside any at-rule. */
function topLevelRootBody(css: string): string | undefined {
	let depth = 0;
	let ruleStart = 0;
	let bodyStart: number | undefined;
	for (let index = 0; index < css.length; index += 1) {
		const character = css[index];
		if (character === '{') {
			if (depth === 0 && css.slice(ruleStart, index).trim() === ':root') bodyStart = index + 1;
			depth += 1;
		} else if (character === '}') {
			depth -= 1;
			if (depth === 0 && bodyStart !== undefined) return css.slice(bodyStart, index);
			if (depth === 0) ruleStart = index + 1;
		}
	}
	return undefined;
}

/** The custom properties the dark `:root` block declares, by name. */
export function darkRootProperties(css: string): Read<ReadonlyMap<string, string>> {
	const body = topLevelRootBody(css.replace(COMMENT, ''));
	if (body === undefined) return unreadable('no top-level :root block');
	const properties = new Map<string, string>();
	for (const [, name, value] of body.matchAll(CUSTOM_PROPERTY)) {
		if (name !== undefined && value !== undefined) properties.set(name, value.trim());
	}
	return read(properties);
}

const VAR_REFERENCE = /^var\(\s*(--[\w-]+)\s*\)$/;

/** A property's final value, following `var()` references until one isn't. */
export function resolveProperty(
	properties: ReadonlyMap<string, string>,
	name: string
): Read<string> {
	const seen = new Set<string>();
	let current = name;
	while (!seen.has(current)) {
		seen.add(current);
		const value = properties.get(current);
		if (value === undefined) return unreadable(`${current} is not declared on the dark :root`);
		const reference = VAR_REFERENCE.exec(value)?.[1];
		if (reference === undefined) return read(value);
		current = reference;
	}
	return unreadable(`${name} refers back to ${current}`);
}

const HEX = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

const RGB = /^rgba?\(\s*([^)]*?)\s*\)$/i;

function hexColor(digits: string): Rgba {
	const full = digits.length <= 4 ? digits.replace(/./g, (digit) => digit + digit) : digits;
	const channel = (index: number): number => parseInt(full.slice(index * 2, index * 2 + 2), 16);
	const alpha = full.length === 8 ? channel(3) / 255 : 1;
	return { red: channel(0), green: channel(1), blue: channel(2), alpha };
}

/** A number or a percentage of `percentOf`, if it lies within 0..`max`. */
function amountWithin(text: string, percentOf: number, max: number): number | undefined {
	const percent = text.endsWith('%');
	const amount = Number(percent ? text.slice(0, -1) : text);
	if (text === '' || !Number.isFinite(amount)) return undefined;
	const value = percent ? (amount / 100) * percentOf : amount;
	return value >= 0 && value <= max ? value : undefined;
}

/** The channel and alpha texts of `r g b [/ a]` or the legacy `r, g, b[, a]`. */
function rgbPieces(inner: string): readonly string[] {
	if (inner.includes(',')) return inner.split(',').map((piece) => piece.trim());
	const [channels = '', alpha, ...rest] = inner.split('/').map((part) => part.trim());
	const pieces = channels.split(/\s+/);
	if (rest.length > 0) return [];
	return alpha === undefined ? pieces : [...pieces, alpha];
}

function rgbColor(inner: string): Read<Rgba> {
	const pieces = rgbPieces(inner);
	if (pieces.length !== 3 && pieces.length !== 4) {
		return unreadable(`rgb(${inner}) has the wrong number of channels`);
	}
	const [red, green, blue] = pieces.slice(0, 3).map((text) => amountWithin(text, 255, 255));
	const alphaText = pieces[3];
	const alpha = alphaText === undefined ? 1 : amountWithin(alphaText, 1, 1);
	if (red === undefined || green === undefined || blue === undefined || alpha === undefined) {
		return unreadable(`rgb(${inner}) has a channel out of range`);
	}
	return read({ red, green, blue, alpha });
}

/** A hex or `rgb()` colour; any other form is reported, not guessed. */
export function parseColor(text: string): Read<Rgba> {
	const value = text.trim();
	const hex = HEX.exec(value)?.[1];
	if (hex !== undefined) return read(hexColor(hex));
	const rgb = RGB.exec(value)?.[1];
	if (rgb !== undefined) return rgbColor(rgb);
	return unreadable(`${value} is not a hex or rgb() colour`);
}

/** `top` painted over an opaque `under`, blended per channel the way a browser blends sRGB. */
export function composite(top: Rgba, under: Rgba): Rgba {
	const blend = (upper: number, lower: number): number =>
		upper * top.alpha + lower * (1 - top.alpha);
	return {
		red: blend(top.red, under.red),
		green: blend(top.green, under.green),
		blue: blend(top.blue, under.blue),
		alpha: 1
	};
}

function linearChannel(channel: number): number {
	const srgb = channel / 255;
	return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
}

/** Relative luminance, 0 for black to 1 for white. */
export function relativeLuminance(color: Rgba): number {
	return (
		0.2126 * linearChannel(color.red) +
		0.7152 * linearChannel(color.green) +
		0.0722 * linearChannel(color.blue)
	);
}

/** The contrast ratio of two opaque colours, 1 to 21, unrounded. */
export function contrastRatio(first: Rgba, second: Rgba): number {
	const one = relativeLuminance(first);
	const other = relativeLuminance(second);
	return (Math.max(one, other) + 0.05) / (Math.min(one, other) + 0.05);
}
