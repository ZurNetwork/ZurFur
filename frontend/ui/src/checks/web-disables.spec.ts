import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** web's source folder, beside ui in the frontend workspace. */
const webSrc = fileURLToPath(new URL('../../../web/src/', import.meta.url));

/** The rules that hold web's import bans: no comment may switch them off. */
const GUARDED_RULES: ReadonlySet<string> = new Set([
	'no-restricted-imports',
	'web/import-specifiers'
]);

/** The directives that switch rules off, each in lower case. */
const DISABLES: ReadonlySet<string> = new Set([
	'eslint-disable',
	'eslint-disable-line',
	'eslint-disable-next-line'
]);

/** Each comment opener and where its comment ends. */
const OPENERS: readonly (readonly [open: string, close: RegExp])[] = [
	['//', /$/mu],
	['/*', /\*\//u],
	['<!--', /-->/u]
];

type Why = 'bare' | 'guarded';

interface Finding {
	readonly comment: string;
	readonly why: Why;
}

/**
 * The text after every comment opener in `text`, up to that comment's end.
 * Each opener is read on its own, wherever it sits, so a string or another
 * comment holding an opener never hides a comment after it.
 */
function commentBodiesIn(text: string): readonly string[] {
	return OPENERS.flatMap(([open, close]) => {
		const bodies: string[] = [];
		for (let at = text.indexOf(open); at !== -1; at = text.indexOf(open, at + 1)) {
			const rest = text.slice(at + open.length);
			const end = close.exec(rest)?.index ?? rest.length;
			bodies.push(rest.slice(0, end));
		}
		return bodies;
	});
}

/**
 * Why a comment body switches a guarded rule off, read as ESLint and Svelte
 * read a directive but in any letter case: a disable with no rule list (a
 * `-- reason` alone is still none), a disable listing a guarded rule, or an
 * inline config naming one (any backslash counts, since its JSON may escape a
 * name). Undefined when it does neither.
 */
function refusalOf(body: string): Why | undefined {
	const [directivePart = ''] = body.split(/\s-{2,}\s/u);
	const trimmed = directivePart.trim();
	const directive = /^(eslint(?:-[a-z]+)*)(?:\s|$)/iu.exec(trimmed)?.[1];
	if (directive === undefined) return undefined;
	const value = trimmed.slice(directive.length).toLowerCase();
	const kind = directive.toLowerCase();
	if (kind === 'eslint') {
		const namesGuarded = [...GUARDED_RULES].some((rule) => value.includes(rule));
		return namesGuarded || value.includes('\\') ? 'guarded' : undefined;
	}
	if (!DISABLES.has(kind)) return undefined;
	const rules = value
		.split(/[\s,]+/u)
		.map((rule) => rule.replaceAll(/['"]/gu, ''))
		.filter((rule) => rule !== '');
	if (rules.length === 0) return 'bare';
	return rules.some((rule) => GUARDED_RULES.has(rule)) ? 'guarded' : undefined;
}

/** Every comment in `text` that switches a guarded rule off. */
function findingsIn(text: string): readonly Finding[] {
	return commentBodiesIn(text).flatMap((comment) => {
		const why = refusalOf(comment);
		return why === undefined ? [] : [{ comment: comment.trim(), why }];
	});
}

/** buf's protobuf output, relative to web/src: the one tree the check skips. */
const PROTOBUF_OUTPUT = 'lib/server/api/generated/';

/** Whether the check reads `relative`, a path under web/src written with `/`. */
function isWalked(relative: string): boolean {
	return !relative.startsWith(PROTOBUF_OUTPUT);
}

/** Every file under web/src the check reads, relative to it and written with `/`. */
function webFiles(): readonly string[] {
	return readdirSync(webSrc, { recursive: true, withFileTypes: true })
		.filter((entry) => entry.isFile())
		.map((entry) => path.relative(webSrc, path.join(entry.parentPath, entry.name)))
		.map((relative) => relative.split(path.sep).join('/'))
		.filter(isWalked);
}

describe("web's disable comments", () => {
	it('reads the files of web/src', () => {
		expect(webFiles()).toContain('hooks.server.ts');
	});

	const walked: readonly string[] = [
		'lib/components/generated/leak.ts',
		'generated/x.ts',
		'lib/server/generated/x.ts',
		'lib/server/api/x.ts',
		'lib/server/api/generated.ts',
		'lib/server/api/generatedx/x.ts',
		'lib/Server/api/generated/x.ts',
		'routes/lib/server/api/generated/x.ts'
	];

	it.each(walked)('reads %s', (relative) => {
		expect(isWalked(relative)).toBe(true);
	});

	it.each(['lib/server/api/generated/x_pb.ts', 'lib/server/api/generated/zurfur/v1/x_pb.ts'])(
		'skips %s',
		(relative) => {
			expect(isWalked(relative)).toBe(false);
		}
	);

	it('refuses a disable in a generated/ folder outside the protobuf output', () => {
		const relative = 'lib/components/generated/leak.ts';
		const leak = "/* eslint-disable */\nimport { Switch } from 'bits-ui';";
		expect(isWalked(relative)).toBe(true);
		expect(findingsIn(leak).map((finding) => finding.why)).toEqual(['bare']);
	});

	it('finds no comment in web/src that switches the import bans off', () => {
		const findings = webFiles().flatMap((relative) =>
			findingsIn(readFileSync(path.join(webSrc, relative), 'utf8')).map(
				({ comment, why }) => `${relative} (${why}): ${comment}`
			)
		);
		expect(findings).toEqual([]);
	});

	const refused: readonly (readonly [label: string, text: string, why: Why])[] = [
		['a bare disable', '/* eslint-disable */', 'bare'],
		['a bare disable over lines', '/*\n\teslint-disable\n*/', 'bare'],
		['a bare -line', "import 'x'; // eslint-disable-line", 'bare'],
		['a bare -next-line with a reason', '// eslint-disable-next-line -- because', 'bare'],
		['a bare disable in markup', '<!-- eslint-disable -->', 'bare'],
		['a bare -next-line in markup', '<!-- eslint-disable-next-line -- because -->', 'bare'],
		['an empty rule list', '// eslint-disable-next-line ,', 'bare'],
		['a disable in another letter case', '/* ESLint-Disable */', 'bare'],
		[
			'a disable behind a string holding an opener',
			"const s = '/*';\n// eslint-disable-line",
			'bare'
		],
		['the import ban', '// eslint-disable-next-line no-restricted-imports', 'guarded'],
		['the specifier rule', '/* eslint-disable web/import-specifiers */', 'guarded'],
		[
			'the import ban among others',
			'// eslint-disable-next-line no-unused-vars, no-restricted-imports -- because',
			'guarded'
		],
		[
			"the import ban's name quoted",
			"/* eslint-disable-line 'no-restricted-imports' */",
			'guarded'
		],
		[
			'the import ban in markup',
			'<!-- eslint-disable-next-line svelte/valid-compile no-restricted-imports -->',
			'guarded'
		],
		['a config comment for the import ban', '/* eslint no-restricted-imports: off */', 'guarded'],
		['a config comment for the specifier rule', '/* eslint web/import-specifiers: 0 */', 'guarded'],
		[
			'a config comment escaping a name',
			'/* eslint "no-restricted\\u002dimports": "off" */',
			'guarded'
		]
	];

	it.each(refused)('refuses %s', (_label, text, why) => {
		expect(findingsIn(text).map((finding) => finding.why)).toContain(why);
	});

	const allowed: readonly (readonly [label: string, text: string])[] = [
		[
			'a disable naming another rule, with a reason',
			'// eslint-disable-next-line no-restricted-syntax -- a boot-time guard'
		],
		[
			'a disable naming another rule in markup',
			'<!-- eslint-disable svelte/no-navigation-without-resolve -->'
		],
		[
			'a disable naming several other rules',
			'/* eslint-disable @typescript-eslint/no-unsafe-call, @typescript-eslint/no-unsafe-return -- untyped */'
		],
		['an enable of the import ban', '// eslint-enable no-restricted-imports'],
		['a config comment for another rule', '/* eslint semi: off */'],
		['a doc comment that mentions a disable', '/** an eslint-disable here would be refused */'],
		['a comment that mentions a disable', '// never write eslint-disable here']
	];

	it.each(allowed)('allows %s', (_label, text) => {
		expect(findingsIn(text)).toEqual([]);
	});
});
