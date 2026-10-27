import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { parseCss } from 'svelte/compiler';
import type { AST } from 'svelte/compiler';
import { PAPER_CLASSES } from '../lib/styles/paper-classes';

const stylesFolder = new URL('../lib/styles/', import.meta.url);
const controlsFolder = new URL('controls/', stylesFolder);

/** Everything in the family folder, by name. */
const folderEntries = readdirSync(controlsFolder).sort();

/** Every family file, by name. */
const familyFiles = folderEntries.filter((name) => name.endsWith('.css'));

function readFamily(name: string): string {
	return readFileSync(new URL(name, controlsFolder), 'utf8');
}

interface Declaration {
	readonly property: string;
	readonly value: string;
}

interface Scan {
	readonly classes: Set<string>;
	readonly declarations: Declaration[];
	readonly atRules: string[];
}

type CssNode = AST.CSS.Atrule | AST.CSS.Rule | AST.CSS.Declaration;

/** Every class name inside a selector, the ones in `:not()`, `:is()` and `:where()` included. */
function collectClasses(node: unknown, into: Set<string>): void {
	if (Array.isArray(node)) {
		for (const item of node) collectClasses(item, into);
		return;
	}
	if (Object(node) !== node) return;
	const fields = Object(node) as Readonly<Record<string, unknown>>;
	if (fields.type === 'ClassSelector' && typeof fields.name === 'string') into.add(fields.name);
	for (const [key, child] of Object.entries(fields)) {
		if (key !== 'metadata') collectClasses(child, into);
	}
}

function visit(nodes: readonly CssNode[], scan: Scan): void {
	for (const node of nodes) {
		switch (node.type) {
			case 'Declaration':
				scan.declarations.push({ property: node.property.toLowerCase(), value: node.value });
				break;
			case 'Rule':
				collectClasses(node.prelude, scan.classes);
				visit(node.block.children, scan);
				break;
			case 'Atrule':
				scan.atRules.push(node.name.toLowerCase());
				if (node.block) visit(node.block.children, scan);
				break;
		}
	}
}

function scanCss(css: string): Scan {
	const scan: Scan = { classes: new Set(), declarations: [], atRules: [] };
	visit(parseCss(css).children, scan);
	return scan;
}

/** The properties a family file may never move: the paper classes and the motion seam own them. */
const MOVING_PROPERTY =
	/(?<![\w-])(?:translate|transform|box-shadow|top|left|width|all)(?![\w-])/gi;

/** A hex colour, or a colour function. */
const RAW_COLOUR =
	/#[0-9a-f]{3,8}(?![\w-])|(?<![\w-])(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/gi;

/** A time, written in ms or s. */
const RAW_TIME = /(?<![\w.-])[+-]?(?:\d+(?:\.\d*)?|\.\d+)m?s(?![\w-])/gi;

/** An easing function, or an easing keyword. */
const RAW_EASING =
	/(?<![\w-])(?:(?:cubic-bezier|steps|linear)\(|(?:ease-in-out|ease-in|ease-out|ease|linear|step-start|step-end)(?![\w(-]))/gi;

/** A length in an absolute or font-relative unit. */
const RAW_LENGTH =
	/(?<![\w.-])[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:px|rem|em|ex|ch|pt|pc|cm|mm|in|q|vh|vw|vmin|vmax|lh)(?![\w-])/gi;

/** The two literal lengths the bundle writes for borders and focus rings. */
const ALLOWED_LENGTHS: readonly string[] = ['1px', '2px'];

/** A bare word in a value, not a function's name nor part of a custom property's. */
const BARE_WORD = /(?<![\w-])[a-z]+(?![\w(-])/gi;

/** The CSS named colours (CSS Color 4), transparent and currentColor aside. */
const NAMED_COLOURS = new Set(
	`aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet
	brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue
	darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange
	darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise
	darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia
	gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo
	ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan
	lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue
	lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon
	mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue
	mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin
	navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen
	paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red
	rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue
	slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white
	whitesmoke yellow yellowgreen`.split(/\s+/)
);

/** What one family file breaks, as readable lines; empty when it keeps every rule. */
function findingsIn(css: string): readonly string[] {
	const scan = scanCss(css);
	const atRules = scan.atRules
		.filter((name) => name.endsWith('keyframes'))
		.map((name) => `@${name}: no animation in a family file`);
	const declarations = scan.declarations.flatMap(({ property, value }): string[] => {
		const found: string[] = [];
		const line = `${property}: ${value}`;
		if (property.replace(/^-\w+-/, '').startsWith('animation')) {
			found.push(`${line}: no animation in a family file`);
		}
		if (property === 'transition' || property === 'transition-property') {
			for (const match of value.matchAll(MOVING_PROPERTY)) {
				found.push(`${line}: transitions ${match[0]}`);
			}
		}
		const isOutline = property === 'outline' || property === 'outline-style';
		if (isOutline && /^\s*(?:none|0)\s*$/i.test(value)) {
			found.push(`${line}: every focus keeps the one ring`);
		}
		if (property === 'outline-width' && /^\s*0\s*$/.test(value)) {
			found.push(`${line}: every focus keeps the one ring`);
		}
		for (const match of value.matchAll(RAW_COLOUR)) found.push(`${line}: raw colour ${match[0]}`);
		for (const match of value.matchAll(RAW_TIME)) found.push(`${line}: raw time ${match[0]}`);
		for (const match of value.matchAll(RAW_EASING)) found.push(`${line}: raw easing ${match[0]}`);
		for (const match of value.matchAll(RAW_LENGTH)) {
			if (!ALLOWED_LENGTHS.includes(match[0])) found.push(`${line}: raw length ${match[0]}`);
		}
		for (const match of value.matchAll(BARE_WORD)) {
			if (NAMED_COLOURS.has(match[0].toLowerCase())) {
				found.push(`${line}: named colour ${match[0]}`);
			}
		}
		return found;
	});
	const paperClasses: readonly string[] = PAPER_CLASSES;
	const classes = [...scan.classes]
		.filter((name) => !name.startsWith('zf-') && !paperClasses.includes(name))
		.map((name) => `.${name}: neither a bundle zf- class nor a paper class`);
	return [...atRules, ...declarations, ...classes];
}

describe('the control family files', () => {
	it('are the seven families', () => {
		expect(familyFiles).toEqual([
			'button.css',
			'check.css',
			'field.css',
			'menu.css',
			'tabs.css',
			'tip.css',
			'toggle.css'
		]);
	});

	it("sit beside nothing but the folder's node", () => {
		expect(folderEntries.filter((name) => !familyFiles.includes(name))).toEqual(['NODE.inf.json']);
	});

	it.each(familyFiles)('%s keeps to tokens, paper classes and colour fades', (name) => {
		expect(findingsIn(readFamily(name))).toEqual([]);
	});

	it('are each imported by the styles entry', () => {
		const entry = readFileSync(new URL('index.ts', stylesFolder), 'utf8');
		const imported = [...entry.matchAll(/^import '\.\/controls\/([^']+)';$/gm)].map(
			(match) => match[1]
		);
		expect(familyFiles.filter((name) => !imported.includes(name))).toEqual([]);
	});
});

describe('the family-file rules', () => {
	it.each([
		['an animation', '.zf-btn { animation: none; }'],
		['keyframes', '@keyframes zf-spin { to { opacity: 0; } }'],
		['a transition of translate', '.zf-btn { transition: translate var(--duration-spring); }'],
		['a transition of transform', '.zf-btn { transition-property: transform; }'],
		['a transition of box-shadow', '.zf-btn { transition: box-shadow var(--duration-fast); }'],
		['a transition of left', '.zf-btn { transition: left var(--duration-fast); }'],
		['a transition of top', '.zf-btn { transition: top var(--duration-fast); }'],
		['a transition of width', '.zf-btn { transition: width var(--duration-fast); }'],
		['a transition of all', '.zf-btn { transition: all var(--duration-fast); }'],
		['outline: none', '.zf-tab:focus-visible { outline: none; }'],
		['outline: 0', '.zf-tab:focus-visible { outline: 0; }'],
		['a hex colour', '.zf-btn { color: #fff; }'],
		['an rgb colour', '.zf-btn { color: rgb(0 0 0); }'],
		['an hsl colour', '.zf-btn { color: hsl(0 0% 0%); }'],
		['a named colour', '.zf-btn { border: 1px solid white; }'],
		['a duration in ms', '.zf-btn { transition: color 100ms var(--easing-standard); }'],
		['a duration in s', '.zf-btn { transition-duration: 0.2s; }'],
		['an easing function', '.zf-btn { transition-timing-function: cubic-bezier(0, 0, 1, 1); }'],
		['an easing keyword', '.zf-btn { transition: color var(--duration-fast) ease-out; }'],
		['a px length', '.zf-btn { width: 16px; }'],
		['a rem length', '.zf-btn { padding: 0.5rem; }'],
		['an em length', '.zf-btn { gap: 1em; }'],
		['a class from elsewhere', '.button { color: var(--color-text); }']
	])('refuse %s', (_, css) => {
		expect(findingsIn(css).length).toBeGreaterThan(0);
	});

	it.each([
		[
			'a colour fade on tokens',
			'.zf-btn { transition: color var(--duration-fast) var(--easing-standard); }'
		],
		['the focus ring', '.zf-btn:focus-visible { outline: 2px solid var(--color-focus); }'],
		['a hairline border', '.zf-btn { border: 1px solid transparent; color: currentColor; }'],
		['a paper class', '.zf-btn.placed:hover { background: var(--color-hover); }'],
		['a percentage and a unitless number', '.zf-btn { left: calc(100% - 2px); opacity: 0.55; }']
	])('allow %s', (_, css) => {
		expect(findingsIn(css)).toEqual([]);
	});
});
