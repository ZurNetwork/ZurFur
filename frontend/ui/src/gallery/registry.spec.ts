import { describe, expect, it } from 'vitest';
import { groupNames } from './groups';
import { buildRegistry, registry } from './registry';

const button = '/src/routes/components/button/meta.ts';
const card = '/src/routes/components/card/meta.ts';

function meta(title: string, group: string): unknown {
	return { meta: { title, group } };
}

describe('buildRegistry', () => {
	it('parses a well-formed page', () => {
		const result = buildRegistry([[button, meta('Button', 'Controls')]]);
		expect(result.refusals).toEqual([]);
		expect(result.entries).toEqual([{ slug: 'button', title: 'Button', group: 'Controls' }]);
	});

	it('orders by group, then title, then slug', () => {
		const result = buildRegistry([
			['/src/routes/components/toast/meta.ts', meta('Toast', 'Surfaces and feedback')],
			['/src/routes/components/zebra/meta.ts', meta('Button', 'Controls')],
			[card, meta('Card', 'Tokens and paper')],
			[button, meta('Button', 'Controls')]
		]);
		expect(result.refusals).toEqual([]);
		expect(result.entries.map((entry) => entry.slug)).toEqual(['card', 'button', 'zebra', 'toast']);
	});

	it('orders the same however the files arrive', () => {
		const files: (readonly [string, unknown])[] = [
			[button, meta('Button', 'Controls')],
			[card, meta('Card', 'Controls')]
		];
		expect(buildRegistry(files.toReversed())).toEqual(buildRegistry(files));
	});

	it.each([
		['/src/routes/meta.ts'],
		['/src/routes/components/meta.ts'],
		['/src/routes/components/button/deeper/meta.ts'],
		['/src/routes/other/button/meta.ts'],
		['/src/routes/components/Button/meta.ts'],
		['/src/routes/components/two--hyphens/meta.ts']
	])('refuses the path %s', (path) => {
		const result = buildRegistry([[path, meta('Button', 'Controls')]]);
		expect(result.entries).toEqual([]);
		expect(result.refusals).toEqual([{ path, reason: 'path-shape' }]);
	});

	it.each([
		['no exports', {}, 'missing-meta'],
		['a module that is not an object', 'text', 'missing-meta'],
		['a meta that is not an object', { meta: 'Button' }, 'malformed-meta'],
		['a meta without a title', { meta: { group: 'Controls' } }, 'malformed-meta'],
		['a blank title', { meta: { title: '  ', group: 'Controls' } }, 'malformed-meta'],
		['a title that is not text', { meta: { title: 3, group: 'Controls' } }, 'malformed-meta'],
		['a meta without a group', { meta: { title: 'Button' } }, 'malformed-meta'],
		['a group that is not text', { meta: { title: 'Button', group: 3 } }, 'malformed-meta'],
		['a group the gallery lacks', { meta: { title: 'Button', group: 'Toys' } }, 'unknown-group']
	])('refuses %s', (_label, module, reason) => {
		const result = buildRegistry([[button, module]]);
		expect(result.entries).toEqual([]);
		expect(result.refusals).toEqual([{ path: button, reason }]);
	});

	it('refuses a slug that an earlier file already holds, and keeps the first', () => {
		const result = buildRegistry([
			[button, meta('Button', 'Controls')],
			[button, meta('Another button', 'Controls')]
		]);
		expect(result.entries.map((entry) => entry.title)).toEqual(['Button']);
		expect(result.refusals).toEqual([{ path: button, reason: 'duplicate-slug' }]);
	});

	it('keeps the good pages beside a refused one', () => {
		const result = buildRegistry([
			[button, meta('Button', 'Controls')],
			[card, {}]
		]);
		expect(result.entries.map((entry) => entry.slug)).toEqual(['button']);
		expect(result.refusals).toEqual([{ path: card, reason: 'missing-meta' }]);
	});
});

describe('groupNames', () => {
	it('lists the groups in nav order', () => {
		expect(groupNames).toEqual([
			'Tokens and paper',
			'Controls',
			'Surfaces and feedback',
			'Navigation and patterns',
			'Motion and sound',
			'Heavy pieces',
			"Pieces for 0.3's screens"
		]);
	});
});

describe('the real registry', () => {
	it('refuses no file', () => {
		expect(registry.refusals).toEqual([]);
	});
});
