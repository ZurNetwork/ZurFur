import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import { denHref, denName, denType, did, handleFromTrusted } from '$lib/types/brand';
import type { BugBrowser, BugPage } from '$lib/types/bug-report-rules';
import { BugReportDraft, offerItems } from './bug-report.svelte';
import ReportBugForm from './ReportBugForm.svelte';
import TopBar from './TopBar.svelte';
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

const alice = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

describe('the top bar before the page’s script runs', () => {
	it('holds "Report a bug" as a plain link to the report page, promising no dialog', () => {
		const html = render(TopBar, {
			props: {
				session: alice,
				trail: [],
				section: 'den',
				drawerOpen: false,
				drawerId: 'drawer',
				onOpenDrawer: () => undefined
			}
		}).body;
		expect(html).toMatch(/<a[^>]*href="\/report-a-bug"[^>]*>[\s\S]*?Report a bug/);
		expect(html).not.toContain('aria-haspopup');
	});
});

describe('the report form before the page’s script runs', () => {
	it('is a plain form posting the report page’s action, its button inside it', () => {
		const offer = {
			page: { kind: 'known' as const, page: '/den' as BugPage },
			pageType: 'My Den' as const,
			browser: 'Mozilla/5.0' as BugBrowser
		};
		const html = render(ReportBugForm, {
			props: {
				draft: new BugReportDraft(),
				items: offerItems(offer, 'web-1'),
				session: alice,
				descriptionId: 'report-a-bug-description',
				sentId: 'sent',
				testStack: 'none',
				submitButton: true
			}
		}).body;
		const form = /<form[^>]*method="POST"[^>]*action="\/report-a-bug"[^>]*>([\s\S]*)<\/form>/.exec(
			html
		);
		expect(form).not.toBeNull();
		const inside = form?.[1] ?? '';
		expect(inside).toContain('name="description"');
		expect(inside).not.toContain('maxlength');
		expect(inside).toMatch(/<input[^>]*name="sendPage"[^>]*checked/);
		expect(inside).toMatch(/<input[^>]*type="hidden"[^>]*name="page"[^>]*value="\/den"/);
		expect(inside).toMatch(/<button[^>]*type="submit"[^>]*>[\s\S]*?Send report/);
		expect(html).toMatch(/aria-live="polite"/);
	});
	it('says, in its fixed lines, that the dev profile’s test stack forgets reports', () => {
		const html = render(ReportBugForm, {
			props: {
				draft: new BugReportDraft(),
				items: [],
				session: alice,
				descriptionId: 'report-a-bug-description',
				sentId: 'sent',
				testStack: 'devProfile',
				submitButton: true
			}
		}).body;

		const fixed = /<div class="report-form__fixed[^"]*">([\s\S]*?)<\/div>/.exec(html);
		expect(fixed?.[1]).toContain('This test stack forgets reports when it stops.');
	});

	it('says nothing about the stack on a real one', () => {
		const html = render(ReportBugForm, {
			props: {
				draft: new BugReportDraft(),
				items: [],
				session: alice,
				descriptionId: 'report-a-bug-description',
				sentId: 'sent',
				testStack: 'none',
				submitButton: true
			}
		}).body;

		expect(html).not.toContain('report-test-stack');
		expect(html).not.toContain('test stack');
	});
});
