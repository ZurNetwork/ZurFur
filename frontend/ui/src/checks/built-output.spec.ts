import { afterAll, describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkBuild, main, type BuiltRule } from './built-output.ts';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** Every fixture folder lives under one temporary folder, removed after the run. */
const scratch = mkdtempSync(path.join(tmpdir(), 'ui-built-output-'));

afterAll(() => {
	rmSync(scratch, { recursive: true, force: true });
});

let fixtureCount = 0;

/** Writes a build folder holding `files` (published path to text) and returns its path. */
function buildFolder(files: Readonly<Record<string, string>>): string {
	fixtureCount += 1;
	const folder = path.join(scratch, `build-${String(fixtureCount)}`);
	for (const [relative, text] of Object.entries(files)) {
		const file = path.join(folder, relative);
		mkdirSync(path.dirname(file), { recursive: true });
		writeFileSync(file, text);
	}
	return folder;
}

/** A page shaped like the gallery build's, with `head` and `body` inside. */
const page = (head = '', body = ''): string =>
	[
		'<!doctype html>',
		'<html lang="en">',
		'\t<head>',
		'\t\t<meta charset="utf-8" />',
		'\t\t<link href="./_app/immutable/assets/0.D1x2.css" rel="stylesheet">',
		'\t\t<link rel="modulepreload" href="./_app/immutable/entry/start.Bx9.js">',
		`\t\t${head}`,
		'\t</head>',
		'\t<body data-sveltekit-preload-data="hover">',
		'\t\t<div style="display: contents"><main><p>{ a brace in text }</p></main>',
		`\t\t\t${body}`,
		'\t\t\t<script>',
		'\t\t\t\t{',
		'\t\t\t\t\t__sveltekit_x = { base: new URL(".", location).pathname.slice(0, -1) };',
		'\t\t\t\t\tconst element = document.currentScript.parentElement;',
		'\t\t\t\t\tPromise.all([import("./_app/immutable/entry/start.Bx9.js")]).then(([kit]) => {',
		'\t\t\t\t\t\tkit.start(element);',
		'\t\t\t\t\t});',
		'\t\t\t\t}',
		'\t\t\t</script>',
		'\t\t</div>',
		'\t</body>',
		'</html>',
		''
	].join('\n');

const cleanCss =
	"@font-face{font-family:x;src:url(./x.woff2) format('woff2')}body{background:url(data:image/png;base64,AAAA)}a{background:url(../img/a.png)}";

/** A build that passes: relative paths, data: URLs and braces as text. */
const cleanBuild = {
	'index.html': page(),
	'components/button.html': page('<link rel="icon" href="../favicon.png">'),
	'_app/immutable/assets/0.D1x2.css': cleanCss,
	'_app/immutable/entry/start.Bx9.js': 'export const start = () => {};'
};

interface Run {
	readonly code: number;
	readonly out: string;
	readonly err: string;
}

function run(argv: readonly string[]): Run {
	let out = '';
	let err = '';
	const code = main(
		argv,
		{
			write: (text: string) => {
				out += text;
			}
		},
		{
			write: (text: string) => {
				err += text;
			}
		}
	);
	return { code, out, err };
}

function rulesOf(folder: string): readonly BuiltRule[] {
	return checkBuild(folder).findings.map((finding) => finding.rule);
}

describe('the built-output check', () => {
	it('passes a clean build', () => {
		const folder = buildFolder(cleanBuild);
		expect(checkBuild(folder)).toEqual({ findings: [], errors: [] });
		expect(run([folder])).toEqual({ code: 0, out: '', err: '' });
	});

	const refused: readonly (readonly [string, Readonly<Record<string, string>>, BuiltRule])[] = [
		[
			'built CSS with a remote @import',
			{ 'a.css': "@import 'https://fonts.googleapis.com/css2?family=x';" },
			'remote-import'
		],
		[
			'built CSS with a remote url()',
			{ 'a.css': 'a{background:url(https://x/a.png)}' },
			'remote-url'
		],
		[
			'built CSS with a protocol-relative url() in upper case',
			{ 'a.css': 'a{background:URL(//x/a.png)}' },
			'remote-url'
		],
		[
			'built CSS with a remote string',
			{ 'a.css': 'a{background:image-set("https://x/a.png" 1x)}' },
			'remote-string'
		],
		['built CSS under another case', { 'A.CSS': 'a{background:url(//x/a.png)}' }, 'remote-url'],
		[
			'built HTML with a protocol-relative src',
			{ 'x.html': page('', '<img src="//x" alt="">') },
			'scheme-attribute'
		],
		[
			'built HTML with a remote href',
			{ 'x.html': page('<link rel="preconnect" href="https://x">') },
			'scheme-attribute'
		],
		[
			'built HTML with a data: href',
			{ 'x.html': page('<link rel="icon" href="data:x">') },
			'scheme-attribute'
		],
		[
			'built HTML with a scheme in a srcset candidate',
			{ 'x.html': page('', '<img srcset="./a.png 1x, blob:x 2x" alt="">') },
			'scheme-attribute'
		],
		[
			'built HTML with a form action',
			{ 'x.html': page('', '<form action="https://x/post"></form>') },
			'scheme-attribute'
		],
		[
			'built HTML with a formaction',
			{ 'x.html': page('', '<button formaction="mailto:a@b">b</button>') },
			'scheme-attribute'
		],
		[
			'built HTML with a poster',
			{ 'x.html': page('', '<video poster="https://x/a.png"></video>') },
			'scheme-attribute'
		],
		[
			'built HTML with an entity-encoded scheme',
			{ 'x.html': page('', '<img src="&#104;ttps://x/a.png" alt="">') },
			'scheme-attribute'
		],
		[
			'built HTML with a remote url() in a style attribute',
			{ 'x.html': page('', '<div style="background:url(https://x/a.png)"></div>') },
			'remote-url'
		],
		[
			'built HTML with a remote @import in a style element',
			{ 'x.html': page('<style>@import "//x/a.css";</style>') },
			'remote-import'
		],
		['built HTML that does not parse', { 'x.html': page('', '<img/src="//x">') }, 'unparsed'],
		[
			'built HTML with a refresh meta',
			{ 'x.html': page('', '<meta http-equiv="refresh" content="0;url=https://x.invalid/leak">') },
			'http-equiv'
		],
		[
			'built HTML with a set-cookie meta',
			{ 'x.html': page('<meta http-equiv="set-cookie" content="a=b">') },
			'http-equiv'
		],
		[
			'built HTML with an x-ua-compatible meta',
			{ 'x.html': page('<meta http-equiv="X-UA-Compatible" content="IE=edge">') },
			'http-equiv'
		],
		[
			'built HTML with an http-equiv that has no value',
			{ 'x.html': page('<meta http-equiv content="0">') },
			'http-equiv'
		],
		[
			'built HTML with an http-equiv that is empty',
			{ 'x.html': page('<meta http-equiv="" content="0">') },
			'http-equiv'
		],
		[
			'built HTML with an http-equiv in braces',
			{ 'x.html': page('<meta http-equiv="{content-security-policy}" content="0">') },
			'http-equiv'
		],
		[
			'built HTML with an http-equiv naming the policy and more',
			{ 'x.html': page('<meta http-equiv="content-security-policy refresh" content="0">') },
			'http-equiv'
		],
		[
			'built HTML with an http-equiv in upper case on another element',
			{ 'x.html': page('', '<div HTTP-EQUIV="refresh"></div>') },
			'http-equiv'
		]
	];

	it.each(refused)('refuses %s', (_label, files, rule) => {
		const folder = buildFolder({ ...cleanBuild, ...files });
		expect(rulesOf(folder)).toContain(rule);
		const { code, out } = run([folder]);
		expect(code).toBe(1);
		const [file] = Object.keys(files);
		expect(out).toContain(path.join(folder, file ?? ''));
		expect(out).toContain(`[${rule}]`);
	});

	it.each([
		['in its own case', 'Content-Security-Policy'],
		['in lower case with spaces around', ' content-security-policy\t']
	])("passes Kit's content-security-policy meta %s", (_label, value) => {
		const meta = `<meta http-equiv="${value}" content="default-src 'self'">`;
		const folder = buildFolder({ ...cleanBuild, 'x.html': page(meta) });
		expect(checkBuild(folder)).toEqual({ findings: [], errors: [] });
	});

	it('refuses a refresh meta beside the policy meta once', () => {
		const policy = `<meta http-equiv="content-security-policy" content="default-src 'self'">`;
		const refresh = '<meta http-equiv="refresh" content="0;url=/x">';
		const folder = buildFolder({ ...cleanBuild, 'x.html': page(policy, refresh) });
		expect(rulesOf(folder)).toEqual(['http-equiv']);
	});

	it('names what it found', () => {
		const folder = buildFolder({ ...cleanBuild, 'a.css': 'a{background:url(https://x/a.png)}' });
		expect(run([folder]).out).toBe(
			`${path.join(folder, 'a.css')}:1:3: [remote-url] background: url(https://x/a.png)\n`
		);
	});

	it('refuses a link, which a server would follow', () => {
		const folder = buildFolder(cleanBuild);
		const outside = buildFolder({ 'evil.css': 'a{background:url(https://x/a.png)}' });
		symlinkSync(path.join(outside, 'evil.css'), path.join(folder, 'linked.css'));
		symlinkSync(outside, path.join(folder, 'linked-folder'));
		expect(rulesOf(folder)).toEqual(['link', 'link']);
		expect(run([folder]).code).toBe(1);
	});

	it.each([
		['a missing folder', () => path.join(scratch, 'missing')],
		['a file', () => path.join(buildFolder({ 'a.css': '' }), 'a.css')],
		['a folder with no page', () => buildFolder({ 'a.css': 'a{color:red}' })],
		[
			'a link to a build',
			() => {
				const link = path.join(scratch, `link-${String(fixtureCount)}`);
				symlinkSync(buildFolder(cleanBuild), link);
				return link;
			}
		]
	])('exits 2 for %s', (_label, folderOf) => {
		const { code, err } = run([folderOf()]);
		expect(code).toBe(2);
		expect(err).toMatch(/^built-output: /);
	});

	it('exits 2 for an unreadable file', () => {
		const folder = buildFolder(cleanBuild);
		const file = path.join(folder, 'index.html');
		chmodSync(file, 0o000);
		const { code, err } = run([folder]);
		chmodSync(file, 0o644);
		expect(code).toBe(2);
		expect(err).toContain(file);
	});

	it.each([
		[[]],
		[['a', 'b']],
		[['--storybook']],
		[['--storybook', 'a', 'b']],
		[['--storybook', '--storybook']],
		[['--kit', 'a']]
	])('exits 2 for the arguments %j', (argv) => {
		const { code, err } = run(argv);
		expect(code).toBe(2);
		expect(err).toContain('usage');
	});

	it.each([
		['a clean build', () => buildFolder(cleanBuild), 0],
		['a remote url()', () => buildFolder({ ...cleanBuild, 'a.css': 'a{background:url(//x)}' }), 1],
		['a missing folder', () => path.join(scratch, 'missing'), 2]
	])('runs under Node itself on %s', (_label, folderOf, expected) => {
		const result = spawnSync(process.execPath, ['src/checks/built-output.ts', folderOf()], {
			cwd: uiRoot,
			encoding: 'utf8'
		});
		expect(result.status, result.stderr).toBe(expected);
	});
});

/** A build shaped like Storybook's static output: the manager, the preview and their scripts. */
const storybookBuild = {
	'index.html': page('<link rel="stylesheet" href="./sb-manager/runtime.css">'),
	'iframe.html': page(
		'',
		'<p>Read <a href="https://storybook.js.org/docs/writing-stories/decorators" target="_blank">the docs</a>.</p>'
	),
	'index.json': '{"v":5,"entries":{}}',
	'project.json': '{}',
	'sb-manager/runtime.js': 'export const run = () => "Zurfur Component Gallery";',
	'assets/iframe-D7dbd-aA.js': 'import "./x.js";',
	'sb-common-assets/nunito-sans-regular.woff2': 'wOF2 ZURFUR_NOT_TEXT',
	'sb-addons/a11y/manager-bundle.js': 'export {};'
};

describe("the built-output check's Storybook mode", () => {
	it('passes a clean Storybook build, its docs link included', () => {
		const folder = buildFolder(storybookBuild);
		expect(checkBuild(folder, 'storybook')).toEqual({ findings: [], errors: [] });
		expect(run(['--storybook', folder])).toEqual({ code: 0, out: '', err: '' });
	});

	it("refuses Storybook's docs link in the Kit gallery's build", () => {
		const folder = buildFolder(storybookBuild);
		expect(rulesOf(folder)).toEqual(['scheme-attribute']);
		expect(run([folder]).code).toBe(1);
	});

	const refused: readonly (readonly [string, Readonly<Record<string, string>>, BuiltRule])[] = [
		[
			'a remote src',
			{ 'iframe.html': page('', '<img src="https://x.example/a.png" alt="">') },
			'scheme-attribute'
		],
		[
			'a remote script',
			{ 'iframe.html': page('<script src="https://x.example/a.js"></script>') },
			'scheme-attribute'
		],
		[
			'a remote <link href>',
			{ 'iframe.html': page('<link rel="stylesheet" href="https://example.org/x.css">') },
			'scheme-attribute'
		],
		[
			'a <link href> into the docs',
			{ 'index.html': page('<link rel="stylesheet" href="https://storybook.js.org/docs/x.css">') },
			'scheme-attribute'
		],
		[
			'a remote CSS url()',
			{ 'sb-manager/a.css': 'a{background:url(https://x.example/a.png)}' },
			'remote-url'
		],
		[
			'a remote CSS @import',
			{ 'sb-manager/a.css': '@import "https://x.example/a.css";' },
			'remote-import'
		],
		[
			'a link to another site',
			{ 'iframe.html': page('', '<a href="https://example.org/">x</a>') },
			'scheme-attribute'
		],
		[
			'a link to the docs over http',
			{ 'iframe.html': page('', '<a href="http://storybook.js.org/docs/x">x</a>') },
			'scheme-attribute'
		],
		[
			'a link beside the docs folder',
			{ 'iframe.html': page('', '<a href="https://storybook.js.org/docsx">x</a>') },
			'scheme-attribute'
		],
		[
			'a link to a host named like Storybook',
			{ 'iframe.html': page('', '<a href="https://storybook.js.org.x.example/docs/">x</a>') },
			'scheme-attribute'
		],
		[
			'a link with credentials',
			{ 'iframe.html': page('', '<a href="https://storybook.js.org@x.example/docs/">x</a>') },
			'scheme-attribute'
		],
		[
			'a docs address on an area',
			{
				'iframe.html': page('', '<map><area href="https://storybook.js.org/docs/x" alt="x"></map>')
			},
			'scheme-attribute'
		],
		[
			'a refresh meta',
			{ 'iframe.html': page('<meta http-equiv="refresh" content="0;url=/x">') },
			'http-equiv'
		],
		['ZURFUR_X in a script', { 'assets/a.js': 'const port = "ZURFUR_X";' }, 'environment-name'],
		[
			'DATABASE_URL in JSON',
			{ 'index.json': '{"DATABASE_URL":"postgres://x"}' },
			'environment-name'
		],
		[
			'an environment name in a source map',
			{ 'assets/a.js.map': '{"sourcesContent":["process.env.ZURFUR_DID_ROOT_KEY"]}' },
			'environment-name'
		],
		['an environment name in Markdown', { 'docs/a.md': 'DATABASE_URL' }, 'environment-name'],
		[
			'an environment name in an SVG',
			{ 'a.svg': '<svg><title>ZURFUR_X</title></svg>' },
			'environment-name'
		],
		[
			'an environment name in a page',
			{ 'iframe.html': page('', '<p>ZURFUR_X</p>') },
			'environment-name'
		],
		['an environment name in a file with no extension', { LICENSE: 'ZURFUR_X' }, 'environment-name']
	];

	it.each(refused)('refuses %s', (_label, files, rule) => {
		const folder = buildFolder({ ...storybookBuild, ...files });
		expect(checkBuild(folder, 'storybook').findings.map((finding) => finding.rule)).toContain(rule);
		const { code, out } = run(['--storybook', folder]);
		expect(code).toBe(1);
		expect(out).toContain(`[${rule}]`);
	});

	it('refuses an environment name in the Kit build too', () => {
		const folder = buildFolder({
			...cleanBuild,
			'_app/env.js': 'export const env = { ZURFUR_X: 1 };'
		});
		expect(rulesOf(folder)).toEqual(['environment-name']);
	});

	it('names where an environment name is', () => {
		const folder = buildFolder({ ...storybookBuild, 'assets/a.js': 'a\nbb DATABASE_URL' });
		expect(run(['--storybook', folder]).out).toBe(
			`${path.join(folder, 'assets/a.js')}:2:4: [environment-name] a bb DATABASE_URL\n`
		);
	});

	it.each(['a.woff2', 'a.WOFF', 'a.png', 'a.ico'])('does not read %s as text', (name) => {
		const folder = buildFolder({ ...storybookBuild, [name]: 'ZURFUR_X DATABASE_URL' });
		expect(checkBuild(folder, 'storybook')).toEqual({ findings: [], errors: [] });
	});

	it.each([
		['a clean Storybook build', () => buildFolder(storybookBuild), 0],
		[
			'a planted environment name',
			() => buildFolder({ ...storybookBuild, 'assets/a.js': 'ZURFUR_SECRET' }),
			1
		]
	])('runs under Node itself on %s', (_label, folderOf, expected) => {
		const result = spawnSync(
			process.execPath,
			['src/checks/built-output.ts', '--storybook', folderOf()],
			{ cwd: uiRoot, encoding: 'utf8' }
		);
		expect(result.status, result.stderr).toBe(expected);
	});
});
