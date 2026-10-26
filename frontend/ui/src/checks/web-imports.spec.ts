import { describe, expect, it } from 'vitest';
import { ESLint, type Linter } from 'eslint';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** web, beside ui in the frontend workspace; its lint config is read from there, never imported. */
const webRoot = fileURLToPath(new URL('../../../web/', import.meta.url));

/** web's own rule for import specifiers, static and dynamic. */
const SPECIFIER_RULE = 'web/import-specifiers';

/** The rules web's import bans live in. */
const importRuleIds: ReadonlySet<string> = new Set([
	'no-restricted-imports',
	'no-restricted-syntax',
	SPECIFIER_RULE
]);

/**
 * web's real config, its import bans unchanged, with type information off so
 * planted text lints without web's generated tsconfig; only the two rules
 * that hold the bans run, since the rest need type information. A comment
 * that switches a ban off is the disable check's to refuse, not this spec's.
 */
const webLinter = new ESLint({
	cwd: webRoot,
	overrideConfig: { languageOptions: { parserOptions: { projectService: false } } },
	ruleFilter: ({ ruleId }) => importRuleIds.has(ruleId)
});

/** web's real config, as its CLI loads it. */
const realWebLinter = new ESLint({ cwd: webRoot });

const UNDECLARED = 'Import only packages web declares';
const EFFECT = 'Effect is server-only';
const DEN_HREF = 'Only the Den path builder';
const NODE_MODULES = 'never through a node_modules path';
const DOT_SEGMENT = 'holds no . or .. segment';
const OUTSIDE = 'never leaves src/';

async function lintWeb(filePath: string, code: string): Promise<readonly Linter.LintMessage[]> {
	const [result] = await webLinter.lintText(code, { filePath });
	if (result === undefined) throw new Error(`no lint result for ${filePath}`);
	const fatal = result.messages.filter((message) => message.fatal === true);
	expect(fatal).toEqual([]);
	return result.messages;
}

async function expectRefusedBecause(filePath: string, code: string, why: string): Promise<void> {
	const messages = await lintWeb(filePath, code);
	const matching = messages.filter(
		(message) =>
			message.ruleId !== null && importRuleIds.has(message.ruleId) && message.message.includes(why)
	);
	expect(matching).not.toEqual([]);
}

async function expectAllowed(filePath: string, code: string): Promise<void> {
	const messages = await lintWeb(filePath, code);
	expect(messages.map((message) => `${message.ruleId ?? ''}: ${message.message}`)).toEqual([]);
}

/** A component: its script holds `body`, then `markup` follows. */
const component = (body: string, markup: string): string =>
	`<script lang="ts">\n${body}\n</script>\n\n${markup}\n`;

/** Paths in web; the file on disk, where there is one, is never read or touched. */
const at = {
	component: 'src/lib/components/ui/Planted.svelte',
	module: 'src/lib/planted.ts',
	route: 'src/routes/planted/+page.ts',
	server: 'src/lib/server/planted.ts',
	serverSpec: 'src/lib/server/planted.spec.ts',
	spec: 'src/lib/planted.spec.ts',
	denPath: 'src/lib/server/den/den-path.ts',
	kitFetch: 'src/lib/testing/kit-fetch.ts',
	config: 'vite.config.ts'
} as const;

type Refusal = readonly [label: string, filePath: string, code: string, why: string];

describe("web's imports", { timeout: 120_000 }, () => {
	it('lints the Den path builder at its real path', () => {
		expect(existsSync(`${webRoot}${at.denPath}`)).toBe(true);
	});

	const refused: readonly Refusal[] = [
		[
			'bits-ui in a component',
			at.component,
			component("\timport { Switch } from 'bits-ui';", '<Switch.Root />'),
			UNDECLARED
		],
		['bits-ui in a module', at.module, "export { Switch } from 'bits-ui';", UNDECLARED],
		['clsx', at.module, "import clsx from 'clsx';\nexport const r = clsx;", UNDECLARED],
		['clsx in a route', at.route, "import clsx from 'clsx';\nexport const r = clsx;", UNDECLARED],
		[
			'@internationalized/date in a component',
			at.component,
			component("\timport { today } from '@internationalized/date';", '<p>{today}</p>'),
			UNDECLARED
		],
		['bits-ui in a server module', at.server, "import 'bits-ui';", UNDECLARED],
		['bits-ui in a server spec', at.serverSpec, "import 'bits-ui';", UNDECLARED],
		['bits-ui in a spec', at.spec, "import 'bits-ui';", UNDECLARED],
		['bits-ui in the Den path builder', at.denPath, "import 'bits-ui';", UNDECLARED],
		['an undeclared package in a config file', at.config, "import 'clsx';", UNDECLARED],
		[
			'a declared package in another letter case',
			at.module,
			"import { Effect } from 'Effect';\nexport const r = Effect;",
			UNDECLARED
		],
		[
			'a name that only starts like a declared one',
			at.module,
			"import 'effect-extra';",
			UNDECLARED
		],
		['a dynamic import of bits-ui', at.module, "export const r = import('bits-ui');", UNDECLARED],
		[
			'effect in a component',
			at.component,
			component("\timport { Effect } from 'effect';", '<p>{Effect.void}</p>'),
			EFFECT
		],
		['effect/Schema in a module', at.module, "import 'effect/Schema';", EFFECT],
		['effect in a spec', at.spec, "import 'effect';", EFFECT],
		[
			'denHref outside the path builder',
			at.module,
			"import { denHref } from '$lib/types/brand';\nexport const r = denHref;",
			DEN_HREF
		],
		[
			'denHref by a relative path',
			at.module,
			"import { denHref } from './types/brand.ts';\nexport const r = denHref;",
			DEN_HREF
		],
		[
			'denHref in a server module',
			at.server,
			"import { denHref } from '$lib/types/brand';\nexport const r = denHref;",
			DEN_HREF
		],
		[
			'a relative path into node_modules',
			at.module,
			"import { Switch } from '../../../node_modules/bits-ui';\nexport const r = Switch;",
			NODE_MODULES
		],
		[
			'a $lib path into node_modules',
			at.module,
			"import * as viaLib from '$lib/../../../node_modules/bits-ui';\nexport const r = viaLib;",
			NODE_MODULES
		],
		[
			'a dynamic import of a template literal',
			at.module,
			'export const lazy = () => import(`bits-ui`);',
			UNDECLARED
		],
		[
			'a declared package reached through node_modules',
			at.module,
			"import 'effect/node_modules/bits-ui';",
			NODE_MODULES
		],
		[
			'a dynamic import into node_modules',
			at.module,
			"export const r = import('./node_modules/bits-ui');",
			NODE_MODULES
		],
		[
			'a template-literal import into node_modules',
			at.component,
			component('\tconst lazy = () => import(`$lib/../../node_modules/bits-ui`);', '<p>{lazy}</p>'),
			NODE_MODULES
		],
		['a $lib path out of src', at.module, "import '$lib/../../x';", OUTSIDE],
		['a relative path out of src', at.module, "import '../../../x';", OUTSIDE],
		['a re-export out of src', at.module, "export { x } from '../../x';", OUTSIDE],
		['a star re-export out of src', at.route, "export * from '../../../x';", OUTSIDE],
		['a dynamic import out of src', at.module, "export const r = import('$lib/../../x');", OUTSIDE],
		[
			'a template-literal import out of src',
			at.server,
			'export const r = import(`../../../x`);',
			OUTSIDE
		],
		['a $app path climbing out', at.module, "import '$app/../../../../bits-ui';", DOT_SEGMENT],
		['an $env path climbing out', at.server, "import '$env/./dynamic/private';", DOT_SEGMENT],
		[
			'a declared package climbing out',
			at.component,
			component("\timport '@zurfur/ui/../bits-ui';", ''),
			DOT_SEGMENT
		],
		['a node: path climbing out', at.server, "import 'node:../x';", DOT_SEGMENT],
		[
			'a dynamic $app path climbing out',
			at.module,
			"export const r = import('$app/../bits-ui');",
			DOT_SEGMENT
		],
		[
			"Kit's internals outside the fetch harness",
			'src/lib/testing/planted.ts',
			"import '../../../../node_modules/@sveltejs/kit/src/runtime/server/fetch.js';",
			NODE_MODULES
		],
		[
			'bits-ui through node_modules in the fetch harness',
			at.kitFetch,
			"import '../../../../node_modules/bits-ui';",
			NODE_MODULES
		],
		[
			"Kit's internals by a dynamic import in the fetch harness",
			at.kitFetch,
			"export const r = import('../../../../node_modules/@sveltejs/kit/src/runtime/server/fetch.js');",
			NODE_MODULES
		],
		[
			'a config file reaching into node_modules',
			at.config,
			"import './node_modules/x';",
			NODE_MODULES
		]
	];

	it.each(refused)('refuses %s', async (_label, filePath, code, why) => {
		await expectRefusedBecause(filePath, code, why);
	});

	const allowed: readonly (readonly [label: string, filePath: string, code: string])[] = [
		[
			'a declared package',
			at.component,
			component("\timport { superForm } from 'sveltekit-superforms';", '<p>{superForm}</p>')
		],
		[
			"a declared package's subpath",
			at.module,
			"import { zod4 } from 'sveltekit-superforms/adapters';\nexport const r = zod4;"
		],
		['$app/paths', at.module, "import { resolve } from '$app/paths';\nexport const r = resolve;"],
		['$lib', at.module, "import { x } from '$lib/x';\nexport const r = x;"],
		['$env', at.server, "import { env } from '$env/dynamic/private';\nexport const r = env;"],
		['a relative path', at.module, "import { x } from './x';\nexport const r = x;"],
		[
			'node:fs in a server module',
			at.server,
			"import { readFileSync } from 'node:fs';\nexport const r = readFileSync;"
		],
		[
			'effect in a server module',
			at.server,
			"import { Effect } from 'effect';\nexport const r = Effect;"
		],
		[
			'@zurfur/ui',
			at.component,
			component("\timport { Icon } from '@zurfur/ui';", '<Icon name="x" />')
		],
		[
			"Kit's internals in the fetch harness",
			at.kitFetch,
			"import '../../../../node_modules/@sveltejs/kit/src/runtime/server/fetch.js';\nimport '../../../../node_modules/@sveltejs/kit/src/runtime/server/cookie.js';"
		],
		['a relative dynamic import', at.module, "export const r = import('./x');"],
		['a template-literal relative import', at.module, 'export const r = import(`./x`);'],
		[
			'a $lib path that stays in src',
			at.module,
			"import { x } from '$lib/../routes/x';\nexport const r = x;"
		],
		[
			'a relative path that stays in src',
			at.module,
			"import { x } from '../x';\nexport const r = x;"
		],
		[
			'a parent path from a route',
			at.route,
			"import { x } from '../../lib/x';\nexport const r = x;"
		],
		[
			'a config file',
			at.config,
			"import { sveltekit } from '@sveltejs/kit/vite';\nexport const r = sveltekit;"
		]
	];

	it.each(allowed)('allows %s', async (_label, filePath, code) => {
		await expectAllowed(filePath, code);
	});

	it.each([
		['a component', at.component],
		['a module', at.module],
		['a route', at.route],
		['a server module', at.server],
		['a server spec', at.serverSpec],
		['a spec', at.spec],
		['the Den path builder', at.denPath],
		['the bare-root fallback', 'src/lib/components/explorer/den-root.ts'],
		['the fetch harness', at.kitFetch],
		['a config file', at.config]
	])('carries the allow-list in %s', async (_label, filePath) => {
		const config = (await realWebLinter.calculateConfigForFile(filePath)) as Linter.Config;
		const entry = config.rules?.['no-restricted-imports'];
		const options: unknown = Array.isArray(entry) ? entry[1] : undefined;
		expect(JSON.stringify(options)).toContain(UNDECLARED);
		const specifierEntry = config.rules?.[SPECIFIER_RULE];
		const severity: unknown = Array.isArray(specifierEntry) ? specifierEntry[0] : specifierEntry;
		expect([2, 'error']).toContain(severity);
	});
});
