import { describe, expect, it } from 'vitest';
import { packageSources, parseSource } from './parse';
import { isRemote, networkFindings, type NetworkRule } from './network';

function rulesOf(path: string, text: string): readonly NetworkRule[] {
	const parsed = parseSource({ path, text });
	return networkFindings(parsed).map((finding) => finding.rule);
}

const component = 'src/lib/Fixture.svelte';
const stylesheet = 'src/lib/fixture.css';
const hostLayout = 'src/routes/+layout.svelte';
const galleryFrame = 'src/gallery/Frame.svelte';

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
	}
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
		expect(paths.filter((path) => path.startsWith('src/checks/'))).toEqual([]);
		const findings = sources.flatMap((source) => networkFindings(parseSource(source)));
		expect(findings).toEqual([]);
	});
});
