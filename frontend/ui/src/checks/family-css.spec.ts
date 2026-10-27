import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { parseCss } from 'svelte/compiler';
import type { AST } from 'svelte/compiler';
import { PAPER_CLASSES } from '../lib/styles/paper-classes';

const libFolder = new URL('../lib/', import.meta.url);
const stylesFolder = new URL('styles/', libFolder);

/** The two family folders, each with exactly the files it holds. */
const FAMILY_FOLDERS = {
	controls: [
		'button.css',
		'check.css',
		'field.css',
		'menu.css',
		'tabs.css',
		'tip.css',
		'toggle.css'
	],
	surfaces: [
		'card.css',
		'dialog.css',
		'label.css',
		'loading.css',
		'mature.css',
		'message.css',
		'pin.css',
		'toast.css'
	]
} as const;

type FamilyFolder = keyof typeof FAMILY_FOLDERS;

const folderNames = Object.keys(FAMILY_FOLDERS) as FamilyFolder[];

/** Everything in a family folder, by name. */
function entriesOf(folder: FamilyFolder): readonly string[] {
	return readdirSync(new URL(`${folder}/`, stylesFolder)).sort();
}

/** What a family folder holds that it shouldn't, or lacks; empty when it is exactly right. */
function folderFindings(
	entries: readonly string[],
	expected: readonly string[]
): readonly string[] {
	const allowed = [...expected, 'NODE.inf.json'];
	const extra = entries
		.filter((name) => !allowed.includes(name))
		.map((name) => `${name}: not a family`);
	const missing = expected
		.filter((name) => !entries.includes(name))
		.map((name) => `${name}: missing`);
	return [...extra, ...missing];
}

/** Every folder under styles/ (anything there but a plain file), by name. */
function foldersInStyles(): readonly string[] {
	return readdirSync(stylesFolder, { withFileTypes: true })
		.filter((entry) => !entry.isFile())
		.map((entry) => entry.name)
		.sort();
}

/** The folders under styles/ that aren't family folders; empty when there are none. */
function strayFolders(names: readonly string[]): readonly string[] {
	const families: readonly string[] = folderNames;
	return names
		.filter((name) => !families.includes(name))
		.map((name) => `${name}/: not a family folder`);
}

/** Every family file, as `folder/name`. */
const familyFiles = folderNames.flatMap((folder) =>
	FAMILY_FOLDERS[folder].map((name) => `${folder}/${name}`)
);

function readStyle(path: string): string {
	return readFileSync(new URL(path, stylesFolder), 'utf8');
}

/** Every stylesheet under `src/lib/`, as a path from it. */
function stylesheetsInLib(): readonly string[] {
	return readdirSync(libFolder, { recursive: true, encoding: 'utf8' })
		.filter((path) => path.endsWith('.css'))
		.map((path) => path.split('\\').join('/'))
		.sort();
}

/** The one stylesheet that may name MatureCover's classes. */
const MATURE_FILE = 'styles/surfaces/mature.css';

/** Where a stylesheet other than mature.css names MatureCover; empty when it doesn't. */
function matureFindings(path: string, css: string): readonly string[] {
	if (path === MATURE_FILE) return [];
	return /zf-mature/i.test(css) ? [`${path}: names zf-mature outside ${MATURE_FILE}`] : [];
}

/** The custom properties tokens.css defines. */
const TOKEN_NAMES = new Set(
	[...readStyle('tokens.css').matchAll(/(--[a-z0-9-]+)\s*:/g)].map((match) => match[1] ?? '')
);

/** The state classes the surfaces' components set. */
const STATE_CLASSES: readonly string[] = ['is-active', 'is-revealed', 'is-hidden'];

interface Declaration {
	readonly property: string;
	readonly value: string;
}

/** A declaration, other than placement, in a rule whose target names no class. */
interface Unplaced extends Declaration {
	/** The rule's selector, its white space folded to single spaces. */
	readonly selector: string;
}

interface Scan {
	readonly source: string;
	readonly classes: Set<string>;
	readonly declarations: Declaration[];
	readonly atRules: string[];
	readonly unplaced: Unplaced[];
}

/** The rule a declaration sits in: its selector, and whether its every target names a class or an element. */
interface Enclosing {
	readonly selector: string;
	readonly named: boolean;
}

/** What a rule may set on a target that names no class: where the piece sits, never how it looks. */
const PLACEMENT_PROPERTIES: readonly string[] = [
	'order',
	'grid-column',
	'grid-column-start',
	'grid-column-end',
	'grid-row',
	'grid-row-start',
	'grid-row-end',
	'grid-area',
	'align-self',
	'justify-self',
	'place-self'
];

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

/**
 * True when a selector's target, its last compound, names a class or an
 * element (`*` names none); a `&` there stands for the enclosing rule's.
 */
function targetIsNamed(selector: AST.CSS.ComplexSelector, enclosingNamed: boolean): boolean {
	const target = selector.children.at(-1)?.selectors ?? [];
	return target.some(
		(simple) =>
			simple.type === 'ClassSelector' ||
			(simple.type === 'TypeSelector' && simple.name !== '*') ||
			(simple.type === 'NestingSelector' && enclosingNamed)
	);
}

function visit(nodes: readonly CssNode[], scan: Scan, enclosing: Enclosing): void {
	for (const node of nodes) {
		switch (node.type) {
			case 'Declaration': {
				const declaration = { property: node.property.toLowerCase(), value: node.value };
				scan.declarations.push(declaration);
				if (!enclosing.named && !PLACEMENT_PROPERTIES.includes(declaration.property)) {
					scan.unplaced.push({ ...declaration, selector: enclosing.selector });
				}
				break;
			}
			case 'Rule': {
				collectClasses(node.prelude, scan.classes);
				const rule: Enclosing = {
					selector: scan.source.slice(node.prelude.start, node.prelude.end).replace(/\s+/g, ' '),
					named: node.prelude.children.every((selector) => targetIsNamed(selector, enclosing.named))
				};
				visit(node.block.children, scan, rule);
				break;
			}
			case 'Atrule':
				scan.atRules.push(node.name.toLowerCase());
				if (node.block) visit(node.block.children, scan, enclosing);
				break;
		}
	}
}

function scanCss(css: string): Scan {
	const scan: Scan = {
		source: css,
		classes: new Set(),
		declarations: [],
		atRules: [],
		unplaced: []
	};
	visit(parseCss(css).children, scan, { selector: '', named: false });
	return scan;
}

/**
 * The rules known to set more than placement on a target that names no class,
 * by family file and selector. The list only shrinks: each entry must still
 * match a rule.
 */
const KNOWN_UNPLACED: readonly { readonly file: string; readonly selector: string }[] = [
	{ file: 'surfaces/toast.css', selector: '.zf-toast-stack, .zf-toast-stack > *' }
];

/** The known selectors for one family file. */
function knownUnplacedIn(path: string): readonly string[] {
	return KNOWN_UNPLACED.filter((known) => known.file === path).map((known) => known.selector);
}

/** The properties a family file may never move: the paper classes and the motion seam own them. */
const MOVING_PROPERTY =
	/(?<![\w-])(?:translate|transform|scale|rotate|opacity|filter|box-shadow|top|left|width|height|all)(?![\w-])/gi;

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

/** What one family file breaks, as readable lines, the known selectors aside; empty when it keeps every rule. */
function findingsIn(css: string, knownSelectors: readonly string[] = []): readonly string[] {
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
		.filter(
			(name) =>
				!name.startsWith('zf-') && !paperClasses.includes(name) && !STATE_CLASSES.includes(name)
		)
		.map((name) => `.${name}: neither a bundle zf- class, a paper class nor a state class`);
	const tokens = scan.declarations
		.flatMap(({ value }) =>
			[...value.matchAll(/var\(\s*(--[\w-]+)/g)].map((match) => match[1] ?? '')
		)
		.filter((name) => !TOKEN_NAMES.has(name))
		.map((name) => `${name}: not a token`);
	const unplaced = scan.unplaced
		.filter(({ selector }) => !knownSelectors.includes(selector))
		.map(
			({ selector, property, value }) =>
				`${selector} { ${property}: ${value} }: only placement on a target that names no class`
		);
	return [...atRules, ...declarations, ...classes, ...tokens, ...unplaced];
}

describe('the family files', () => {
	it.each(folderNames)('%s holds exactly its families, beside nothing but its node', (folder) => {
		expect(folderFindings(entriesOf(folder), FAMILY_FOLDERS[folder])).toEqual([]);
	});

	it('sit in no folder under styles/ but the family folders', () => {
		expect(strayFolders(foldersInStyles())).toEqual([]);
	});

	it.each(familyFiles)('%s keeps to tokens, paper and state classes, and colour fades', (path) => {
		expect(findingsIn(readStyle(path), knownUnplacedIn(path))).toEqual([]);
	});

	it.each(KNOWN_UNPLACED)('still need $file’s known rule $selector', ({ file, selector }) => {
		const found = scanCss(readStyle(file)).unplaced.map((unplaced) => unplaced.selector);
		expect(found).toContain(selector);
	});

	it('are each imported by the styles entry', () => {
		const entry = readStyle('index.ts');
		const imported = [...entry.matchAll(/^import '\.\/((?:controls|surfaces)\/[^']+)';$/gm)].map(
			(match) => match[1]
		);
		expect(familyFiles.filter((path) => !imported.includes(path))).toEqual([]);
	});
});

describe("MatureCover's classes", () => {
	it('are named in no stylesheet under src/lib but mature.css', () => {
		const sheets = stylesheetsInLib();
		expect(sheets).toContain(MATURE_FILE);
		const found = sheets.flatMap((path) =>
			matureFindings(path, readFileSync(new URL(path, libFolder), 'utf8'))
		);
		expect(found).toEqual([]);
	});

	it('are named in mature.css, so the scan reads the right file', () => {
		expect(readStyle('surfaces/mature.css')).toContain('.zf-mature');
	});

	it('refuse a .zf-mature__art rule in card.css', () => {
		const css = '.zf-mature__art { filter: none; }';
		expect(matureFindings('styles/surfaces/card.css', css).length).toBeGreaterThan(0);
	});

	it('refuse a class list that reaches into MatureCover', () => {
		const css = '[class*="zf-mature"] { filter: none; }';
		expect(matureFindings('styles/paper.css', css).length).toBeGreaterThan(0);
	});
});

describe('the family folders', () => {
	it('refuse a ninth file among the surfaces', () => {
		const entries = [...FAMILY_FOLDERS.surfaces, 'NODE.inf.json', 'extra.css'];
		expect(folderFindings(entries, FAMILY_FOLDERS.surfaces)).toEqual(['extra.css: not a family']);
	});

	it('refuse a third folder under styles/', () => {
		expect(strayFolders(['controls', 'layout', 'surfaces'])).toEqual([
			'layout/: not a family folder'
		]);
	});

	it('refuse a missing family', () => {
		const entries = FAMILY_FOLDERS.controls.filter((name) => name !== 'tip.css');
		expect(folderFindings(entries, FAMILY_FOLDERS.controls)).toEqual(['tip.css: missing']);
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
		['a transition of scale', '.zf-pin { transition: scale var(--duration-fast); }'],
		['a transition of rotate', '.zf-spinner { transition-property: rotate; }'],
		['a transition of opacity', '.zf-backdrop { transition: opacity var(--duration-dim); }'],
		['a transition of filter', '.zf-mature__art { transition: filter var(--duration-slow); }'],
		['a transition of height', '.zf-skeleton { transition: height var(--duration-base); }'],
		[
			'keyframes in a surface file',
			'.zf-skeleton { display: block; } @keyframes zf-breathe { to { opacity: 0.55; } }'
		],
		['an unknown token', '.zf-card { padding: var(--space-7); }'],
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
		['a class from elsewhere', '.button { color: var(--color-text); }'],
		['a size on every child', '.zf-dialog__actions > * { min-height: var(--size-control-lg); }'],
		[
			'a colour on a child picked by place',
			'.zf-dialog__actions > :nth-last-child(2) { color: var(--color-text); }'
		],
		['a margin beside a placement', '.zf-card > * { grid-column: 1 / -1; margin: 0; }'],
		['a child styled by a pseudo-class over a class', '.zf-card :is(.zf-btn) { margin: 0; }'],
		['a child styled by an attribute', '.zf-card > [hidden] { display: block; }'],
		[
			'a child styled in a media query',
			'@media (pointer: coarse) { .zf-card > * { padding: 0; } }'
		],
		['a child styled in a nested rule', '.zf-card { & > * { color: var(--color-text); } }']
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
		['a state class', '.zf-pin.is-active { scale: 1.15; }'],
		['a percentage and a unitless number', '.zf-btn { left: calc(100% - 2px); opacity: 0.55; }'],
		[
			'an order on a child picked by place',
			'.zf-dialog__actions > :nth-last-child(2) { order: 1; }'
		],
		[
			'the grid placements on every child',
			'.zf-card > * { grid-column: 1 / -1; grid-row-start: 2; align-self: start; place-self: end; }'
		],
		['a child named by its element', '.zf-card img { display: block; }'],
		['a nested state of its own class', '.zf-card { &:hover { color: var(--color-text); } }']
	])('allow %s', (_, css) => {
		expect(findingsIn(css)).toEqual([]);
	});
});
