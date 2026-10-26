import { describe, expect, it } from 'vitest';
import { ESLint, type Linter } from 'eslint';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** web, beside ui in the frontend workspace; its lint config is read from there, never imported. */
const webRoot = fileURLToPath(new URL('../../../web/', import.meta.url));

/** The two rules web's import bans live in. */
const importRuleIds: ReadonlySet<string> = new Set([
	'no-restricted-imports',
	'no-restricted-syntax'
]);

/**
 * web's real config, its import bans unchanged, with type information off so
 * planted text lints without web's generated tsconfig; only the two rules
 * that hold the bans run, since the rest need type information.
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
		['a relative dynamic import', at.module, "export const r = import('./x');"],
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
		['a config file', at.config]
	])('carries the allow-list in %s', async (_label, filePath) => {
		const config = (await realWebLinter.calculateConfigForFile(filePath)) as Linter.Config;
		const entry = config.rules?.['no-restricted-imports'];
		const options: unknown = Array.isArray(entry) ? entry[1] : undefined;
		expect(JSON.stringify(options)).toContain(UNDECLARED);
	});
});
