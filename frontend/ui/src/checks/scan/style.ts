/**
 * The style scan: motion times and easings live in the token file only.
 * Every other `.svelte`, `.html` and `.css` file refuses a time (`200ms`,
 * `0.2s`, `-1s`) in any declaration, an easing function (`cubic-bezier(`,
 * `steps(`, `linear(`) in any declaration, and an easing keyword (`ease`,
 * `linear`, `step-start`, …) in a `transition*`, `animation*` or custom
 * property. It reads `<style>` blocks, `<style>` elements, `.css` files,
 * `style` attributes and `style:` directives.
 *
 * Not caught: a component's own `transition:`, `in:` or `out:` function with
 * a numeric duration, and a time in a `style` attribute or directive built
 * from an expression's run-time value (static text around an expression, and
 * string literals in it, are read).
 */
import { cssValueParts, type ParsedSource, type Place } from './parse';

/** The one file where motion times and easings may be written. */
export const tokenFile = 'src/lib/styles/tokens.css';

export type StyleRule = 'time' | 'easing-function' | 'easing-keyword' | 'unparsed';

export interface StyleFinding {
	readonly rule: StyleRule;
	readonly found: string;
	readonly place: Place;
}

interface ValueRule {
	readonly rule: Exclude<StyleRule, 'unparsed'>;
	readonly appliesTo: (property: string) => boolean;
	/** A global pattern, matched against the value's code (strings, comments and URLs blanked). */
	readonly pattern: RegExp;
}

const anyProperty = (): boolean => true;

/** `transition*`, `animation*` (vendor prefix aside) and custom properties. */
function isMotionProperty(property: string): boolean {
	const bare = property.toLowerCase().replace(/^-(webkit|moz|ms|o)-/, '');
	return bare.startsWith('transition') || bare.startsWith('animation') || bare.startsWith('--');
}

const valueRules: readonly ValueRule[] = [
	{
		rule: 'time',
		appliesTo: anyProperty,
		pattern: /(?<![\w.-])[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?m?s(?![\w-])/gi
	},
	{
		rule: 'easing-function',
		appliesTo: anyProperty,
		pattern: /(?<![\w-])(?:cubic-bezier|steps|linear)\(/gi
	},
	{
		rule: 'easing-keyword',
		appliesTo: isMotionProperty,
		pattern:
			/(?<![\w-])(?:ease-in-out|ease-in|ease-out|ease|linear|step-start|step-end)(?![\w(-])/gi
	}
];

/** The style findings in one parsed file; the token file has none. */
export function styleFindings(source: ParsedSource): readonly StyleFinding[] {
	if (source.path === tokenFile) return [];
	const unparsed = source.unparsed.map((failure): StyleFinding => ({
		rule: 'unparsed',
		found: failure.reason,
		place: failure.place
	}));
	const inValues = source.declarations.flatMap((declaration) => {
		const { code } = cssValueParts(declaration.value);
		return valueRules
			.filter((valueRule) => valueRule.appliesTo(declaration.property))
			.flatMap((valueRule) =>
				[...code.matchAll(valueRule.pattern)].map((match): StyleFinding => ({
					rule: valueRule.rule,
					found: `${declaration.property}: ${match[0]}`,
					place: declaration.place
				}))
			);
	});
	return [...unparsed, ...inValues];
}
