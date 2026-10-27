import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { entryPath, exportedComponents, storiedComponent } from './scan/exports';
import { packageSources, type SourceFile } from './scan/parse';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** UI-3a's exported controls other than Button: the only names the pending list may hold. */
const UI_3A_CONTROLS: readonly string[] = [
	'IconButton',
	'Icon',
	'TextInput',
	'Select',
	'Checkbox',
	'Toggle',
	'Tabs',
	'PillSwitch',
	'Menu',
	'DateField'
];

/**
 * The exported controls that have no stories yet. It only shrinks: each name
 * leaves once its stories land, and no name joins it.
 */
const PENDING: readonly string[] = [];

export type CoverageRefusalReason =
	| 'an exported component without stories'
	| 'an exported component without its contract spec'
	| 'a pending component that has stories'
	| "a pending name outside UI-3a's controls";

export interface CoverageRefusal {
	readonly reason: CoverageRefusalReason;
	readonly name: string;
}

export interface CoverageInput {
	/** The public entry's text. */
	readonly entry: string;
	/** Every stories file under src/stories/. */
	readonly stories: readonly SourceFile[];
	/** True when a file exists, by its path relative to the package. */
	readonly hasFile: (relative: string) => boolean;
	readonly pending: readonly string[];
}

/**
 * Every refusal of the coverage rule: each exported component has a stories
 * file naming it and its `Name.svelte.spec.ts` beside it; the pending list
 * exempts from stories only, holds only UI-3a's controls, and never a
 * component that has stories.
 */
export function coverageRefusals(input: CoverageInput): readonly CoverageRefusal[] {
	const storied = new Set(input.stories.flatMap((file) => storiedComponent(file) ?? []));
	const exported = exportedComponents(input.entry);
	const refusal = (reason: CoverageRefusalReason, name: string): CoverageRefusal => ({
		reason,
		name
	});
	return [
		...exported
			.filter(({ name }) => !storied.has(name) && !input.pending.includes(name))
			.map(({ name }) => refusal('an exported component without stories', name)),
		...exported
			.filter(({ file }) => !input.hasFile(`${file}.spec.ts`))
			.map(({ name }) => refusal('an exported component without its contract spec', name)),
		...input.pending
			.filter((name) => storied.has(name))
			.map((name) => refusal('a pending component that has stories', name)),
		...input.pending
			.filter((name) => !UI_3A_CONTROLS.includes(name))
			.map((name) => refusal("a pending name outside UI-3a's controls", name))
	];
}

/** A stories file at `src/stories/controls/<fileName>`, naming `component` from `from`. */
function storiesOf(fileName: string, component: string, from = '@zurfur/ui'): SourceFile {
	const text = [
		'<script module lang="ts">',
		"\timport { defineMeta } from '@storybook/addon-svelte-csf';",
		`\timport { ${component} } from '${from}';`,
		'',
		`\tconst { Story } = defineMeta({ title: 'Controls/${component}', component: ${component} });`,
		'</script>',
		'',
		'<Story name="Plain" />',
		''
	].join('\n');
	return { path: `src/stories/controls/${fileName}`, text };
}

/** A public entry exporting each name from `./controls/<Name>.svelte`, beside a plain export. */
function entryOf(...names: readonly string[]): string {
	const lines = names.map(
		(name) => `export { default as ${name} } from './controls/${name}.svelte';`
	);
	return ["export { parseSkin, type Skin } from './styles/environment.ts';", ...lines].join('\n');
}

const everyFileExists = (): boolean => true;

describe('story coverage', () => {
	it('reads the components the public entry exports, and nothing else it exports', () => {
		const entry = [
			entryOf('Button', 'Toggle'),
			"export { default as Icon } from './icons/Icon.svelte';",
			"export { ICON_NAMES, isIconName, type IconName } from './icons/icons.ts';",
			"export type { default as Typed } from './controls/Typed.svelte';",
			"export { default } from './controls/Unnamed.svelte';",
			"export * from './controls/Starred.svelte';"
		].join('\n');
		expect(exportedComponents(entry)).toEqual([
			{ name: 'Button', file: 'src/lib/controls/Button.svelte' },
			{ name: 'Toggle', file: 'src/lib/controls/Toggle.svelte' },
			{ name: 'Icon', file: 'src/lib/icons/Icon.svelte' }
		]);
	});

	it.each([
		['a plain import', storiesOf('Button.stories.svelte', 'Button'), 'Button'],
		[
			'an aliased import',
			{
				path: 'src/stories/controls/Button.stories.svelte',
				text: "<script module lang=\"ts\">\n\timport { defineMeta } from '@storybook/addon-svelte-csf';\n\timport { Button as Shown } from '@zurfur/ui';\n\n\tconst { Story } = defineMeta({ component: Shown });\n</script>\n"
			},
			'Button'
		],
		[
			'a component imported by path',
			storiesOf('Button.stories.svelte', 'Button', '../../lib/controls/Button.svelte'),
			undefined
		],
		[
			'no defineMeta',
			{
				path: 'src/stories/controls/X.stories.svelte',
				text: '<script module lang="ts">\n</script>\n'
			},
			undefined
		],
		[
			'a file that does not parse',
			{ path: 'src/stories/controls/X.stories.svelte', text: '<script module lang="ts">' },
			undefined
		]
	] as const)('reads the component of a stories file with %s', (_label, file, expected) => {
		expect(storiedComponent(file)).toBe(expected);
	});

	it('passes a component with its stories and its contract spec', () => {
		const refusals = coverageRefusals({
			entry: entryOf('Button'),
			stories: [storiesOf('Button.stories.svelte', 'Button')],
			hasFile: everyFileExists,
			pending: PENDING
		});
		expect(refusals).toEqual([]);
	});

	it('passes a pending component without stories', () => {
		const refusals = coverageRefusals({
			entry: entryOf('Toggle'),
			stories: [],
			hasFile: everyFileExists,
			pending: ['Toggle']
		});
		expect(refusals).toEqual([]);
	});

	it.each([
		[
			'a new export without stories',
			{ entry: entryOf('Banner'), stories: [], pending: [] },
			[{ reason: 'an exported component without stories', name: 'Banner' }]
		],
		[
			'a pending name that has stories',
			{
				entry: entryOf('Toggle'),
				stories: [storiesOf('Toggle.stories.svelte', 'Toggle')],
				pending: ['Toggle']
			},
			[{ reason: 'a pending component that has stories', name: 'Toggle' }]
		],
		[
			'Banner in the pending list',
			{ entry: entryOf(), stories: [], pending: ['Banner'] },
			[{ reason: "a pending name outside UI-3a's controls", name: 'Banner' }]
		],
		[
			'Button in the pending list',
			{ entry: entryOf('Button'), stories: [], pending: ['Button'] },
			[{ reason: "a pending name outside UI-3a's controls", name: 'Button' }]
		],
		[
			'an export whose stories file names another component',
			{
				entry: entryOf('Button', 'Toggle'),
				stories: [storiesOf('Button.stories.svelte', 'Toggle')],
				pending: []
			},
			[{ reason: 'an exported component without stories', name: 'Button' }]
		],
		[
			'an export whose stories import it by path',
			{
				entry: entryOf('Button'),
				stories: [storiesOf('Button.stories.svelte', 'Button', '../../lib/controls/Button.svelte')],
				pending: []
			},
			[{ reason: 'an exported component without stories', name: 'Button' }]
		]
	] as const)('refuses %s', (_label, input, expected) => {
		expect(coverageRefusals({ ...input, hasFile: everyFileExists })).toEqual(expected);
	});

	it('refuses an export without its contract spec, pending or not', () => {
		const refusals = coverageRefusals({
			entry: entryOf('Button', 'Toggle'),
			stories: [storiesOf('Button.stories.svelte', 'Button')],
			hasFile: (relative) => relative === 'src/lib/controls/Button.svelte',
			pending: ['Toggle']
		});
		expect(refusals).toEqual([
			{ reason: 'an exported component without its contract spec', name: 'Button' },
			{ reason: 'an exported component without its contract spec', name: 'Toggle' }
		]);
	});

	it("pins the pending list to UI-3a's controls, each once", () => {
		expect(PENDING.filter((name) => !UI_3A_CONTROLS.includes(name))).toEqual([]);
		expect(new Set(PENDING).size).toBe(PENDING.length);
	});

	it('passes the real public entry and stories', () => {
		const stories = packageSources().filter(
			(source) => source.path.startsWith('src/stories/') && source.path.endsWith('.stories.svelte')
		);
		const refusals = coverageRefusals({
			entry: readFileSync(`${uiRoot}${entryPath}`, 'utf8'),
			stories,
			hasFile: (relative) => existsSync(`${uiRoot}${relative}`),
			pending: PENDING
		});
		expect(refusals).toEqual([]);
	});
});
