import { describe, expect, it } from 'vitest';
import { docComments, libCodeSources, packageSources, parseSource } from './parse';
import {
	docCommentFindings,
	isRemote,
	networkFindings,
	schemeOf,
	type NetworkRule
} from './network';

function rulesOf(path: string, text: string): readonly NetworkRule[] {
	const parsed = parseSource({ path, text });
	return networkFindings(parsed).map((finding) => finding.rule);
}

const component = 'src/lib/Fixture.svelte';
const stylesheet = 'src/lib/fixture.css';
const hostLayout = 'src/routes/+layout.svelte';
const galleryFrame = 'src/gallery/Frame.svelte';
const shell = 'src/app.html';

/** A page shell shaped like the real one, with `head` and `body` inside. */
const shellWith = (head: string, body = ''): string =>
	`<!doctype html>\n<html lang="en">\n\t<head>\n\t\t<meta charset="utf-8" />\n\t\t${head}\n\t\t%sveltekit.head%\n\t</head>\n\t<body data-sveltekit-preload-data="hover">\n\t\t<div style="display: contents">%sveltekit.body%</div>${body}\n\t</body>\n</html>\n`;

interface Fixture {
	readonly name: string;
	readonly path: string;
	readonly text: string;
	readonly rule: NetworkRule;
}

const refused: readonly Fixture[] = [
	{
		name: 'an https url() in a style block',
		path: component,
		text: '<style>div { background: url(https://x/a.png); }</style>',
		rule: 'remote-url'
	},
	{
		name: 'a protocol-relative url()',
		path: component,
		text: '<style>div { background: url(//x/a.png); }</style>',
		rule: 'remote-url'
	},
	{
		name: 'a quoted http url()',
		path: component,
		text: '<style>div { background: url( "http://x/a.png" ); }</style>',
		rule: 'remote-url'
	},
	{
		name: 'an escaped url()',
		path: component,
		text: '<style>div { background: url(\\68ttps://x/a.png); }</style>',
		rule: 'remote-url'
	},
	{
		name: 'a url() in a font face',
		path: stylesheet,
		text: '@font-face { font-family: x; src: url(https://x/a.woff2); }',
		rule: 'remote-url'
	},
	{
		name: 'a url() in a .css file',
		path: stylesheet,
		text: 'a { background: url(https://x/a.png); }',
		rule: 'remote-url'
	},
	{
		name: 'a url() in a style attribute',
		path: component,
		text: '<div style="background: url(https://x/a.png)"></div>',
		rule: 'remote-url'
	},
	{
		name: 'a url() in a style directive',
		path: component,
		text: '<div style:background-image="url(//x/a.png)"></div>',
		rule: 'remote-url'
	},
	{
		name: 'a url() in a style element',
		path: component,
		text: '<div><style>p { background: url(https://x/a.png); }</style></div>',
		rule: 'remote-url'
	},
	{
		name: 'a string in image-set()',
		path: component,
		text: '<style>div { background: image-set("https://x/a.png" 1x); }</style>',
		rule: 'remote-string'
	},
	{
		name: '@import url()',
		path: component,
		text: '<style>@import url(https://x/a.css);</style>',
		rule: 'remote-import'
	},
	{
		name: '@import of a string',
		path: stylesheet,
		text: "@import '//x/a.css';",
		rule: 'remote-import'
	},
	{
		name: 'an img src',
		path: component,
		text: '<img src="https://x/a.png" alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'a protocol-relative href',
		path: component,
		text: '<a href="//x/a">a</a>',
		rule: 'remote-attribute'
	},
	{
		name: 'a later srcset candidate',
		path: component,
		text: '<img srcset="a.png 1x, https://x/b.png 2x" alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'an imagesrcset candidate',
		path: hostLayout,
		text: '<link rel="preload" as="image" imagesrcset="https://x/b.png 2x" />',
		rule: 'remote-attribute'
	},
	{
		name: 'a form action',
		path: component,
		text: '<form action="https://x/post"></form>',
		rule: 'remote-attribute'
	},
	{
		name: 'a button formaction',
		path: component,
		text: '<button formaction="https://x/post">b</button>',
		rule: 'remote-attribute'
	},
	{
		name: 'a video poster',
		path: component,
		text: '<video poster="https://x/a.png"></video>',
		rule: 'remote-attribute'
	},
	{
		name: 'an object data',
		path: component,
		text: '<object data="https://x/a.svg" title="a"></object>',
		rule: 'remote-attribute'
	},
	{
		name: 'an xlink:href',
		path: component,
		text: '<svg><use xlink:href="https://x/a.svg#i"></use></svg>',
		rule: 'remote-attribute'
	},
	{
		name: 'a ping list',
		path: component,
		text: '<a href="/a" ping="/b https://x/c">a</a>',
		rule: 'remote-attribute'
	},
	{
		name: 'a background attribute',
		path: component,
		text: '<table background="https://x/a.png"></table>',
		rule: 'remote-attribute'
	},
	{
		name: 'a string literal in an expression',
		path: component,
		text: "<img src={big ? 'https://x/a.png' : '/a.png'} alt=\"\" />",
		rule: 'remote-attribute'
	},
	{
		name: 'a template literal in an expression',
		path: component,
		text: '<img src={`https://${host}/a.png`} alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'static text before an expression',
		path: component,
		text: '<img src="https://{host}/a.png" alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'an upper-case attribute and scheme',
		path: component,
		text: '<img SRC="HTTPS://x/a.png" alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'an entity-encoded scheme',
		path: component,
		text: '<img src="&#104;ttps://x/a.png" alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'backslashes',
		path: component,
		text: '<img src="\\\\x/a.png" alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'a tab inside the scheme',
		path: component,
		text: '<img src="ht\ttps://x/a.png" alt="" />',
		rule: 'remote-attribute'
	},
	{
		name: 'a component prop named href',
		path: component,
		text: '<Link href="https://x/a">a</Link>',
		rule: 'remote-attribute'
	},
	{
		name: 'a script in an .html file',
		path: 'src/app.html',
		text: '<head><script src="https://x/a.js"></script></head>',
		rule: 'remote-attribute'
	},
	{
		name: '@import of a font host',
		path: stylesheet,
		text: "@import 'https://fonts.googleapis.com/x';",
		rule: 'remote-import'
	},
	{
		name: '@import of a protocol-relative string',
		path: stylesheet,
		text: '@import "//x/y.css";',
		rule: 'remote-import'
	},
	{
		name: '@import with an escaped url(',
		path: stylesheet,
		text: '@import u\\72l(https://x/a.css);',
		rule: 'remote-import'
	},
	{
		name: 'an escaped @import',
		path: stylesheet,
		text: '@\\69mport "https://x/a.css";',
		rule: 'remote-import'
	},
	{
		name: '@import of a package path',
		path: stylesheet,
		text: "@import 'pkg/x.css';",
		rule: 'non-relative-import'
	},
	{
		name: '@import url() of a bare name',
		path: stylesheet,
		text: '@import url(x.css);',
		rule: 'non-relative-import'
	},
	{
		name: '@import of a root path',
		path: stylesheet,
		text: "@import '/x.css';",
		rule: 'non-relative-import'
	},
	{
		name: '@import of a data: sheet',
		path: stylesheet,
		text: "@import 'data:text/css,a{}';",
		rule: 'non-relative-import'
	},
	{
		name: '@import in a style block',
		path: component,
		text: "<style>@import 'pkg/x.css';</style>",
		rule: 'non-relative-import'
	},
	{
		name: '@import with no target',
		path: stylesheet,
		text: '@import x;',
		rule: 'non-relative-import'
	},
	{
		name: 'an upper-case URL(',
		path: stylesheet,
		text: 'a { background: URL(https://x/a.png); }',
		rule: 'remote-url'
	},
	{
		name: 'an escaped url( name',
		path: stylesheet,
		text: 'a { background: u\\72l(https://x/a.png); }',
		rule: 'remote-url'
	},
	{
		name: 'an image-set() string',
		path: stylesheet,
		text: 'a { background: image-set("https://a.test/x" 1x); }',
		rule: 'remote-string'
	},
	{
		name: 'a protocol-relative content string',
		path: stylesheet,
		text: 'a::before { content: "//x"; }',
		rule: 'remote-string'
	},
	{
		name: 'an ftp: url()',
		path: stylesheet,
		text: 'a { background: url(ftp://x/a.png); }',
		rule: 'scheme-url'
	},
	{
		name: 'a blob: url()',
		path: component,
		text: '<style>div { background: url(blob:x); }</style>',
		rule: 'scheme-url'
	},
	{
		name: 'a javascript: url() behind a tab',
		path: stylesheet,
		text: 'a { background: url(" \\9 javascript:x"); }',
		rule: 'scheme-url'
	},
	{
		name: 'a scheme in a custom property string',
		path: stylesheet,
		text: ':root { --x: "mailto:a@b"; }',
		rule: 'scheme-string'
	},
	{
		name: 'a link to another origin in the shell',
		path: shell,
		text: shellWith('<link rel="stylesheet" href="https://x" />'),
		rule: 'remote-attribute'
	},
	{
		name: 'a data: link in the shell',
		path: shell,
		text: shellWith('<link rel="icon" href="data:x" />'),
		rule: 'shell-scheme'
	},
	{
		name: 'a mailto: link in the shell',
		path: shell,
		text: shellWith('', '<a href="mailto:a@b">a</a>'),
		rule: 'shell-scheme'
	},
	{
		name: 'a poster with a scheme in the shell',
		path: shell,
		text: shellWith('', '<video poster="blob:x"></video>'),
		rule: 'shell-scheme'
	},
	{
		name: 'a form action with a scheme in the shell',
		path: shell,
		text: shellWith('', '<form action=" JAVASCRIPT:x"></form>'),
		rule: 'shell-scheme'
	},
	{
		name: 'markup in braces in the shell',
		path: shell,
		text: shellWith('', `<div>{'<img src="//x" alt="">'}</div>`),
		rule: 'remote-attribute'
	},
	{
		name: 'markup in an {@html} in the shell',
		path: shell,
		text: shellWith(`{@html '<link rel="icon" href="data:x">'}`),
		rule: 'shell-scheme'
	},
	{
		name: 'markup in a block in the shell',
		path: shell,
		text: shellWith('{#if false}<link rel="icon" href="data:x" />{/if}'),
		rule: 'shell-scheme'
	},
	{
		name: 'a remote string in a shell script',
		path: shell,
		text: shellWith("<script>import('https://x/a.js');</script>"),
		rule: 'remote-script-string'
	},
	{
		name: 'an escaped remote string in a shell script',
		path: shell,
		text: shellWith("<script>new Image().src = '\\x2f/x/a.png';</script>"),
		rule: 'remote-script-string'
	},
	{
		name: 'a shell script that is not JavaScript',
		path: shell,
		text: shellWith('<script type="importmap">{"imports": {"x": "./x.js"}}</script>'),
		rule: 'unparsed'
	},
	{
		name: 'a shell holding a brace stand-in',
		path: shell,
		text: shellWith('<title>\uFDD0</title>'),
		rule: 'unparsed'
	},
	{
		name: 'a remote url() in a shell style',
		path: shell,
		text: shellWith('<style>body { background: url(https://x/a.png); }</style>'),
		rule: 'remote-url'
	},
	{
		name: 'a svelte:head in lib',
		path: component,
		text: '<svelte:head><link rel="icon" href="/a.png" /></svelte:head>',
		rule: 'head-outside-host'
	},
	{
		name: 'a svelte:head in lib holding only a title',
		path: component,
		text: '<svelte:head><title>x</title></svelte:head>',
		rule: 'head-outside-host'
	},
	{
		name: 'a svelte:head in a nested lib folder',
		path: 'src/lib/deep/Fixture.svelte',
		text: '<svelte:head><title>x</title></svelte:head>',
		rule: 'head-outside-host'
	},
	{
		name: 'a svelte:head in a folder named like the host',
		path: 'src/lib/routes/Fixture.svelte',
		text: '<svelte:head><title>x</title></svelte:head>',
		rule: 'head-outside-host'
	},
	{
		name: "a script in the host's head",
		path: hostLayout,
		text: '<svelte:head><script src="/a.js"></script></svelte:head>',
		rule: 'head-content'
	},
	{
		name: "a link in the gallery frame's head",
		path: galleryFrame,
		text: '<svelte:head><link rel="stylesheet" href="/a.css" /></svelte:head>',
		rule: 'head-content'
	},
	{
		name: "a meta without a name in the host's head",
		path: hostLayout,
		text: '<svelte:head><meta charset="utf-8" /></svelte:head>',
		rule: 'head-content'
	},
	{
		name: "a refresh meta in the host's head",
		path: hostLayout,
		text: '<svelte:head><meta name="x" http-equiv="refresh" content="0" /></svelte:head>',
		rule: 'head-content'
	},
	{
		name: "a style in the host's head",
		path: hostLayout,
		text: '<svelte:head><style>p { color: red; }</style></svelte:head>',
		rule: 'head-content'
	},
	{
		name: "a block in the host's head",
		path: hostLayout,
		text: '<svelte:head>{#if a}<title>x</title>{/if}</svelte:head>',
		rule: 'head-content'
	},
	{
		name: "text in the host's head",
		path: hostLayout,
		text: '<svelte:head>hello</svelte:head>',
		rule: 'head-content'
	},
	{
		name: "a refresh meta in a route's body markup",
		path: 'src/routes/components/paper/+page.svelte',
		text: '<main><meta http-equiv="refresh" content="0;url=https://x.invalid/leak" /></main>',
		rule: 'http-equiv'
	},
	{
		name: 'a refresh meta in a lib component',
		path: component,
		text: '<meta http-equiv="refresh" content="0;url=/a" />',
		rule: 'http-equiv'
	},
	{
		name: 'an http-equiv in mixed case',
		path: component,
		text: '<meta HTTP-Equiv="refresh" content="0" />',
		rule: 'http-equiv'
	},
	{
		name: 'an http-equiv on a svelte:element',
		path: component,
		text: '<svelte:element this="meta" http-equiv="refresh" content="0" />',
		rule: 'http-equiv'
	},
	{
		name: 'an http-equiv with no value',
		path: component,
		text: '<meta http-equiv content="0" />',
		rule: 'http-equiv'
	},
	{
		name: 'an http-equiv holding an expression',
		path: component,
		text: '<meta http-equiv={kind} content="0" />',
		rule: 'http-equiv'
	},
	{
		name: 'a content-security-policy meta in a component',
		path: component,
		text: '<meta http-equiv="content-security-policy" content="default-src *" />',
		rule: 'http-equiv'
	},
	{
		name: "an http-equiv in the host's head",
		path: hostLayout,
		text: '<svelte:head><meta name="x" http-equiv="refresh" content="0" /></svelte:head>',
		rule: 'http-equiv'
	},
	{
		name: 'a refresh meta in the shell',
		path: shell,
		text: shellWith('<meta http-equiv="refresh" content="0;url=/a" />'),
		rule: 'http-equiv'
	}
];

const passed: readonly Omit<Fixture, 'rule'>[] = [
	{
		name: 'a data: url()',
		path: component,
		text: '<style>div { background: url(data:image/png;base64,AAAA); }</style>'
	},
	{
		name: 'relative url()s',
		path: component,
		text: "<style>div { background: url(/a.png), url(a.png), url('../b.png'); }</style>"
	},
	{ name: 'a relative @import', path: stylesheet, text: "@import './tokens.css';" },
	{ name: 'a parent @import', path: stylesheet, text: "@import '../x.css';" },
	{ name: 'a relative @import url()', path: stylesheet, text: '@import url(./x.css) layer(base);' },
	{
		name: 'a relative font url()',
		path: stylesheet,
		text: "@font-face { font-family: x; src: url(./x.woff2) format('woff2'); }"
	},
	{
		name: 'a data: url() in a .css file',
		path: stylesheet,
		text: 'a { background: url(data:image/svg+xml;utf8,<svg/>); }'
	},
	{ name: 'a fragment url()', path: stylesheet, text: 'a { filter: url(#f); }' },
	{
		name: 'strings that name no scheme',
		path: stylesheet,
		text: 'a { font-family: "Inter Display", sans-serif; content: "Step 1 of 2"; }'
	},
	{
		name: 'a word ending in url',
		path: stylesheet,
		text: 'a { --curl: x; background: myurl(x); }'
	},
	{
		name: 'a shell like the real one',
		path: shell,
		text: shellWith('<meta name="viewport" content="width=device-width, initial-scale=1" />')
	},
	{
		name: 'a shell with a boot script and a style',
		path: shell,
		text: shellWith(
			"<script>\n\t\t\t// read the skin before paint\n\t\t\ttry {\n\t\t\t\tconst skin = { name: localStorage.getItem('zurfur.skin') ?? 'paper' };\n\t\t\t\tdocument.documentElement.dataset.skin = skin.name;\n\t\t\t} catch {}\n\t\t</script>\n\t\t<style>html { background: Canvas; }</style>"
		)
	},
	{
		name: 'a relative link in the shell',
		path: shell,
		text: shellWith('<link rel="icon" href="./favicon.png" />')
	},
	{
		name: 'relative attributes',
		path: component,
		text: '<a href="/a">a</a><a href="#b">b</a><img src="a.png" srcset="a.png 1x, /b.png 2x" alt="" />'
	},
	{ name: 'a mailto link', path: component, text: '<a href="mailto:a@example.org">a</a>' },
	{
		name: 'a template literal led by an expression',
		path: component,
		text: '<a href={`${root}components/${slug}`}>a</a>'
	},
	{
		name: "the host's title",
		path: hostLayout,
		text: '<svelte:head>\n\t<!-- the page --><title>Zurfur · {page}</title>\n</svelte:head>'
	},
	{
		name: "a named meta in the gallery frame's head",
		path: galleryFrame,
		text: '<svelte:head><meta name="description" content="x" /></svelte:head>'
	},
	{
		name: 'a non-URL attribute holding a URL',
		path: component,
		text: '<p title="https://x/a">a</p>'
	},
	{ name: 'a named meta in markup', path: component, text: '<meta name="x" content="y" />' }
];

describe('isRemote', () => {
	it.each([
		'https://x',
		'http://x',
		'HTTP://x',
		'//x',
		'\\\\x',
		'/\\x',
		' \thttps://x',
		'h\nttps://x'
	])('reads %j as remote', (url) => {
		expect(isRemote(url)).toBe(true);
	});

	it.each(['/a', 'a.png', '#a', 'data:image/png;base64,AA', 'mailto:a@b', '', '/'])(
		'reads %j as local',
		(url) => {
			expect(isRemote(url)).toBe(false);
		}
	);
});

describe('schemeOf', () => {
	it.each([
		['https://x', 'https'],
		['data:x', 'data'],
		[' \tFTP://x', 'ftp'],
		['ja\nvascript:x', 'javascript'],
		['a+b.c-d:x', 'a+b.c-d']
	])('reads %j as %j', (url, scheme) => {
		expect(schemeOf(url)).toBe(scheme);
	});

	it.each(['./a:b', '/a:b', 'a/b:c', '1a:b', '#a:b', '', '//x', 'a b:c'])(
		'reads no scheme in %j',
		(url) => {
			expect(schemeOf(url)).toBeUndefined();
		}
	);
});

describe('the network scan', () => {
	it.each(refused)('refuses $name', ({ path, text, rule }) => {
		expect(rulesOf(path, text)).toContain(rule);
	});

	it.each(passed)('passes $name', ({ path, text }) => {
		expect(rulesOf(path, text)).toEqual([]);
	});

	it('refuses a file that does not parse, rather than crashing', () => {
		expect(rulesOf(component, '<img src="https://x" {</div>')).toEqual(['unparsed']);
		expect(rulesOf(stylesheet, 'a { background: url(https://x)')).toEqual(['unparsed']);
	});

	it('finds every real file clean', () => {
		const sources = packageSources();
		const paths = sources.map((source) => source.path);
		expect(paths).toContain('src/routes/+layout.svelte');
		expect(paths).toContain('src/app.html');
		expect(paths).toContain('src/stories/frame/StoryFrame.svelte');
		expect(paths.filter((path) => path.startsWith('src/checks/'))).toEqual([]);
		const findings = sources.flatMap((source) => networkFindings(parseSource(source)));
		expect(findings).toEqual([]);
	});
});

function docRulesOf(path: string, text: string): readonly NetworkRule[] {
	return docCommentFindings(path, docComments({ path, text })).map((finding) => finding.rule);
}

const libModule = 'src/lib/fixture.ts';
const libComponent = 'src/lib/Fixture.svelte';

describe('the doc comments under src/lib/', () => {
	it.each([
		[
			'an image tag in a JSDoc comment',
			libModule,
			'/** Shows <img src=x onerror=alert(1)> here. */\nexport const x = 1;',
			'doc-tag'
		],
		[
			'a link in a JSDoc comment',
			libModule,
			'/** See https://example.org for more. */\nexport const x = 1;',
			'doc-url'
		],
		[
			'a protocol-relative link',
			libModule,
			'/** See //example.org/x. */\nexport const x = 1;',
			'doc-url'
		],
		[
			'an http link in upper case',
			libModule,
			'/** See HTTP://example.org. */\nexport const x = 1;',
			'doc-url'
		],
		['a closing tag', libModule, '/** Ends here </div> */\nexport const x = 1;', 'doc-tag'],
		['an HTML comment', libModule, '/** Hidden <!-- x --> */\nexport const x = 1;', 'doc-tag'],
		[
			'a tag on a property',
			libModule,
			'export interface P {\n\t/** The <b>label</b>. */\n\treadonly label: string;\n}',
			'doc-tag'
		],
		['a tag in a trailing comment', libModule, 'export const x = 1; /** <script> */', 'doc-tag'],
		[
			'a tag after an escaped backtick',
			libModule,
			'/** A \\`<img src=x>` here. */\nexport const x = 1;',
			'doc-tag'
		],
		[
			'a tag inside a double-backtick span',
			libModule,
			'/** A ``<img src=x>`` here. */\nexport const x = 1;',
			'doc-tag'
		],
		[
			'a tag in a span across lines',
			libModule,
			'/**\n * A `<img\n * src=x>` here.\n */\nexport const x = 1;',
			'doc-tag'
		],
		[
			'a URL inside a code span',
			libModule,
			'/** Fetches `https://example.org`. */\nexport const x = 1;',
			'doc-url'
		],
		[
			'a tag in a component prop comment',
			libComponent,
			'<script lang="ts">\n\tinterface Props {\n\t\t/** The <img src=x> label. */\n\t\tlabel: string;\n\t}\n\tlet { label }: Props = $props();\n</script>\n\n<p>{label}</p>',
			'doc-tag'
		],
		[
			'a link in a module script',
			libComponent,
			'<script module lang="ts">\n\t/** See https://example.org. */\n\texport const x = 1;\n</script>',
			'doc-url'
		],
		[
			'a tag in a @component comment',
			libComponent,
			'<!--\n@component\nA <img src=x onerror=alert(1)> button.\n-->\n<button>x</button>',
			'doc-tag'
		],
		[
			'a link in a nested @component comment',
			libComponent,
			'<div><!-- @component See https://example.org --></div>',
			'doc-url'
		],
		['a component that does not parse', libComponent, '<script>\n/** x */\n</scr', 'unparsed']
	] as const)('refuses %s', (_label, path, text, rule) => {
		expect(docRulesOf(path, text)).toContain(rule);
	});

	it.each([
		[
			'plain words',
			libModule,
			'/** The paper moves, in the order the gallery lists them. */\nexport const x = 1;'
		],
		[
			'a tag-like word in a code span',
			libModule,
			'/** The attribute on `<html>` or any element below it. */\nexport const x = 1;'
		],
		['a comparison', libModule, '/** True when a < b and b > 2. */\nexport const x = 1;'],
		['a line comment holding a URL', libModule, '// See https://example.org\nexport const x = 1;'],
		[
			'a block comment that is not JSDoc',
			libModule,
			'/* See https://example.org */\nexport const x = 1;'
		],
		['a URL in a string', libModule, "export const x = '/** https://example.org */';"],
		['an HTML comment without the tag', libComponent, '<!-- See https://example.org -->\n<p>x</p>'],
		[
			'plain words in a @component comment',
			libComponent,
			'<!--\n@component\nA button that sends a request.\n-->\n<button>x</button>'
		],
		[
			'a doc comment outside lib',
			'src/gallery/fixture.ts',
			'/** <img src=x> https://example.org */\nexport const x = 1;'
		]
	])('passes %s', (_label, path, text) => {
		expect(docRulesOf(path, text)).toEqual([]);
	});

	it('finds the real doc comments clean', () => {
		const sources = libCodeSources();
		const paths = sources.map((source) => source.path);
		expect(paths).toContain('src/lib/index.ts');
		const read = sources.map((source) => docComments(source));
		expect(read.flatMap((comments) => comments.comments).length).toBeGreaterThan(0);
		const findings = sources.flatMap((source, index) =>
			docCommentFindings(source.path, read[index] ?? { comments: [], unparsed: [] })
		);
		expect(findings).toEqual([]);
	});
});
