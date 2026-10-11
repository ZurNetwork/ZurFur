import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import { denHref, denName, denType, did } from '$lib/types/brand';
import Tree from './Tree.svelte';
import ViewOptions from './ViewOptions.svelte';
import { TreeMemory } from './tree-memory.svelte';

const ROOT = denHref('/den');

/** The tree as the server renders it, before the page's script runs. */
function serverHtml(): string {
	const memory = new TreeMemory(did('did:plc:alice'));
	memory.loaded(
		ROOT,
		[
			{
				view: 'open',
				href: denHref('/den/accounts'),
				name: denName('accounts'),
				type: denType('user.accounts'),
				kind: 'directory',
				mounted: false,
				ownLevel: 'private',
				contentNotShown: false,
				removed: undefined
			}
		],
		undefined
	);
	memory.expanded.add(ROOT);
	const current = { href: undefined, parentHref: undefined, entry: undefined };
	return render(Tree, { props: { memory, current, fallbackRootHref: ROOT } }).body;
}

describe('Tree before the page’s script runs', () => {
	it('is plain nested lists of links that Tab reaches one by one', () => {
		const html = serverHtml();
		expect(html).toContain('href="/den/accounts"');
		expect(html).not.toContain('role="tree"');
		expect(html).not.toContain('role="treeitem"');
		expect(html).not.toContain('role="group"');
		expect(html).not.toContain('role="none"');
		expect(html).not.toContain('tabindex=');
	});
});

describe('the tree’s "More…" before the page’s script runs', () => {
	it('is a plain link to the next page', () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		memory.loaded(ROOT, [], denHref('/den?pageToken=p1.4'));
		memory.loaded(
			ROOT,
			[
				{
					view: 'open',
					href: denHref('/den/accounts'),
					name: denName('accounts'),
					type: denType('user.accounts'),
					kind: 'directory',
					mounted: false,
					ownLevel: 'private',
					contentNotShown: false,
					removed: undefined
				}
			],
			denHref('/den?pageToken=p1.4')
		);
		memory.expanded.add(ROOT);
		const current = { href: undefined, parentHref: undefined, entry: undefined };
		const html = render(Tree, { props: { memory, current, fallbackRootHref: ROOT } }).body;
		expect(html).toMatch(/<a[^>]*href="\/den\?pageToken=p1\.4"[^>]*>[\s\S]*?More…/);
	});
});

describe('ViewOptions before the page’s script runs', () => {
	it('is a plain link to the other address, built by the server', () => {
		const flagLinks = {
			on: denHref('/den/commissions?includeDeleted=true'),
			off: denHref('/den/commissions')
		};
		const html = render(ViewOptions, { props: { includeDeleted: false, flagLinks } }).body;
		expect(html).toContain('href="/den/commissions?includeDeleted=true"');
		expect(html).toContain('Show archived and deactivated');
		expect(html).not.toContain('role="switch"');
	});
});
