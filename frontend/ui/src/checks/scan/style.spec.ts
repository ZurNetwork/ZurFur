import { describe, expect, it } from 'vitest';
import { packageSources, parseSource } from './parse';
import { styleFindings, tokenFile, type StyleRule } from './style';

function rulesOf(path: string, text: string): readonly StyleRule[] {
	const parsed = parseSource({ path, text });
	return styleFindings(parsed).map((finding) => finding.rule);
}

const component = 'src/lib/Fixture.svelte';
const stylesheet = 'src/lib/fixture.css';

interface Fixture {
	readonly name: string;
	readonly path: string;
	readonly text: string;
}

const refusedTimes: readonly Fixture[] = [
	{
		name: 'a duration in a style block',
		path: component,
		text: '<style>div { transition: 200ms; }</style>'
	},
	{
		name: 'seconds with a fraction',
		path: component,
		text: '<style>div { transition: opacity 0.2s; }</style>'
	},
	{
		name: 'a negative delay',
		path: component,
		text: '<style>div { animation-delay: -1s; }</style>'
	},
	{
		name: 'an upper-case unit',
		path: component,
		text: '<style>div { transition: opacity 1S; }</style>'
	},
	{
		name: 'a time in a custom property',
		path: component,
		text: '<style>:root { --fast: 200ms; }</style>'
	},
	{
		name: 'a time as a var() fallback',
		path: component,
		text: '<style>div { transition-duration: var(--fast, 200ms); }</style>'
	},
	{
		name: 'a vendor-prefixed transition',
		path: component,
		text: '<style>div { -webkit-transition: 1s; }</style>'
	},
	{
		name: 'a time in a nested rule',
		path: component,
		text: '<style>div { &:hover { transition: 1s; } }</style>'
	},
	{
		name: 'a time in a media block',
		path: component,
		text: '<style>@media (min-width: 1px) { div { transition: 1s; } }</style>'
	},
	{ name: 'a style attribute', path: component, text: '<div style="transition: 200ms"></div>' },
	{
		name: 'a style attribute around an expression',
		path: component,
		text: '<div style="transition-duration: {d}ms"></div>'
	},
	{
		name: 'a template literal in a style attribute',
		path: component,
		text: '<div style={`transition: ${d}ms`}></div>'
	},
	{
		name: 'a style directive',
		path: component,
		text: '<div style:transition-duration="200ms"></div>'
	},
	{
		name: 'a string literal in a style directive',
		path: component,
		text: "<div style:transition-duration={fast ? '100ms' : '200ms'}></div>"
	},
	{
		name: 'a style element inside the markup',
		path: component,
		text: '<div><style>p { transition: 1s; }</style></div>'
	},
	{ name: 'a .css file', path: stylesheet, text: 'a { animation: x 1s; }' },
	{
		name: "the token file's text under another name",
		path: 'src/lib/styles/other.css',
		text: ':root { --fast: 200ms; }'
	},
	{
		name: 'a time in an .html file',
		path: 'src/app.html',
		text: '<div style="transition: 1s"></div>'
	}
];

const refusedFunctions: readonly Fixture[] = [
	{
		name: 'cubic-bezier(',
		path: component,
		text: '<style>div { transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }</style>'
	},
	{
		name: 'steps(',
		path: component,
		text: '<style>div { animation-timing-function: steps(4, end); }</style>'
	},
	{
		name: 'linear(',
		path: component,
		text: '<style>div { transition-timing-function: linear(0, 0.5, 1); }</style>'
	},
	{
		name: 'an easing function in a custom property',
		path: stylesheet,
		text: ':root { --ease: cubic-bezier(0.2, 0, 0, 1); }'
	}
];

const easingKeywords = [
	'ease',
	'ease-in',
	'ease-out',
	'ease-in-out',
	'linear',
	'step-start',
	'step-end'
] as const;

const refusedKeywords: readonly Fixture[] = [
	...easingKeywords.map((keyword) => ({
		name: `${keyword} in transition-timing-function`,
		path: component,
		text: `<style>div { transition-timing-function: ${keyword}; }</style>`
	})),
	{
		name: 'a keyword in an animation shorthand',
		path: component,
		text: '<style>div { animation: spin linear infinite; }</style>'
	},
	{
		name: 'a keyword in a custom property',
		path: stylesheet,
		text: ':root { --ease: ease-in-out; }'
	},
	{
		name: 'a keyword in a style attribute',
		path: component,
		text: '<div style="transition-timing-function: EASE-IN"></div>'
	}
];

const passed: readonly Fixture[] = [
	{
		name: 'the token file',
		path: tokenFile,
		text: ':root { --fast: 200ms; --ease: cubic-bezier(0.2, 0, 0, 1); --out: ease-out; }'
	},
	{
		name: 'var() references',
		path: component,
		text: '<style>div { transition: opacity var(--fast) var(--ease); }</style>'
	},
	{
		name: 'linear-gradient(',
		path: component,
		text: '<style>div { background: linear-gradient(red, blue); }</style>'
	},
	{
		name: '@keyframes percentages',
		path: component,
		text: '<style>@keyframes spin { 0% { opacity: 0; } 50.5% { opacity: 1; } }</style>'
	},
	{
		name: 'strings',
		path: component,
		text: '<style>div::after { content: "200ms ease linear(1)"; }</style>'
	},
	{
		name: 'comments',
		path: component,
		text: '<style>div { color: red; /* 200ms ease */ }</style>'
	},
	{
		name: 'a time-like URL',
		path: component,
		text: '<style>div { background: url(/frames/1s.png); }</style>'
	},
	{
		name: 'an easing-like name outside motion properties',
		path: component,
		text: '<style>div { font-family: linear; }</style>'
	},
	{
		name: 'an animation name that starts like a keyword',
		path: component,
		text: '<style>div { animation-name: ease-fade; }</style>'
	},
	{
		name: 'lengths and numbers',
		path: stylesheet,
		text: 'a { width: 10px; z-index: 2; margin: 1em 0.5rem; }'
	},
	{ name: 'time-like identifiers', path: stylesheet, text: 'a { grid-area: area-1s; }' },
	{
		name: 'a plain style attribute',
		path: component,
		text: '<div style="display: contents"></div>'
	}
];

describe('the style scan', () => {
	it.each(refusedTimes)('refuses a time: $name', ({ path, text }) => {
		expect(rulesOf(path, text)).toContain('time');
	});

	it.each(refusedFunctions)('refuses an easing function: $name', ({ path, text }) => {
		expect(rulesOf(path, text)).toContain('easing-function');
	});

	it.each(refusedKeywords)('refuses an easing keyword: $name', ({ path, text }) => {
		expect(rulesOf(path, text)).toContain('easing-keyword');
	});

	it.each(passed)('passes $name', ({ path, text }) => {
		expect(rulesOf(path, text)).toEqual([]);
	});

	it('refuses a file that does not parse, rather than crashing', () => {
		expect(rulesOf(component, '<div {</div>')).toEqual(['unparsed']);
		expect(rulesOf(stylesheet, 'a { b: c')).toEqual(['unparsed']);
		expect(rulesOf(component, '<div style="a: b} c {d: e"></div>')).toEqual(['unparsed']);
	});

	it('places a finding on its line and column', () => {
		const parsed = parseSource({
			path: component,
			text: '<p></p>\n<style>\n\tdiv { transition: 1s; }\n</style>'
		});
		expect(styleFindings(parsed)).toEqual([
			{ rule: 'time', found: 'transition: 1s', place: { path: component, line: 3, column: 8 } }
		]);
	});

	it('finds every real file clean', () => {
		const sources = packageSources();
		expect(sources.map((source) => source.path)).toContain('src/routes/+layout.svelte');
		const findings = sources.flatMap((source) => styleFindings(parseSource(source)));
		expect(findings).toEqual([]);
	});
});
