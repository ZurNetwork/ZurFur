import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { DenBody, DenEntry, DenOutcome, DenPageData, OpenEntry } from '$lib/api/den';
import { denHref, denName, denType } from '$lib/types/brand';
import DenPane from './DenPane.svelte';

/** An open entry at `href`. */
function open(href: string, name: string, extra: Partial<OpenEntry> = {}): OpenEntry {
	return {
		view: 'open',
		href: denHref(href),
		name: denName(name),
		type: denType('folder'),
		kind: 'directory',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined,
		...extra
	};
}

/** Page data around `outcome`. */
function data(outcome: DenOutcome): DenPageData {
	return {
		kind: 'denPage',
		outcome,
		rootHref: denHref('/den'),
		includeDeleted: false,
		continued: false,
		ancestors: [],
		title: 'x',
		trail: []
	};
}

/** An open page of `node` holding `body`, below the root. */
function openPage(node: OpenEntry, body: DenBody, root = false): DenPageData {
	const crumbs = root ? [] : [{ href: denHref('/den'), name: denName('Alice') }];
	return data({
		outcome: 'page',
		page: { view: 'open', node, crumbs, body },
		flagLinks: { on: node.href, off: node.href }
	});
}

const untitled = open('/den/commissions/c1', 'Untitled', { mounted: true });

describe('DenPane', () => {
	it('titles the root "My Den" and lists its entries, folders first', async () => {
		const entries: readonly DenEntry[] = [
			open('/den/notes', 'notes', { kind: 'file' }),
			open('/den/accounts', 'accounts')
		];
		render(DenPane, {
			data: openPage(open('/den', 'Alice'), { body: 'listing', entries, more: undefined }, true)
		});

		await expect.element(page.getByRole('heading', { name: 'My Den' })).toBeInTheDocument();
		const rows = page.getByTestId('entry-list').element().querySelectorAll('li');
		expect([...rows].map((row) => row.textContent.trim().split(/\s+/)[0])).toEqual([
			'accounts',
			'notes'
		]);
	});

	it('shows the open node’s kind, own visibility, mounted marker and soft-delete word', async () => {
		const archived = open('/den/commissions/w', 'Winter YCH', {
			mounted: true,
			removed: 'archived'
		});
		render(DenPane, {
			data: openPage(archived, { body: 'listing', entries: [], more: undefined })
		});

		const header = page.getByTestId('node-header');
		await expect.element(header.getByTestId('node-kind')).toHaveTextContent('Folder');
		await expect.element(header.getByTestId('visibility')).toHaveTextContent('Private');
		await expect.element(header.getByText('Mounted')).toBeInTheDocument();
		await expect.element(header.getByTestId('badge')).toHaveTextContent('Archived');
	});

	it('says "Deactivated" for a deactivated Account, never "Archived"', async () => {
		const former = open('/den/accounts/did:plc:x', 'Former Studio', { removed: 'deactivated' });
		render(DenPane, { data: openPage(former, { body: 'listing', entries: [], more: undefined }) });

		await expect.element(page.getByTestId('badge')).toHaveTextContent('Deactivated');
	});

	it('says "Set aside" for a soft-delete word this build doesn’t know', async () => {
		const odd = open('/den/x', 'Odd', { removed: 'unknown' });
		render(DenPane, { data: openPage(odd, { body: 'listing', entries: [], more: undefined }) });

		await expect.element(page.getByTestId('badge')).toHaveTextContent('Set aside');
	});

	it('says "This folder is empty." for an empty folder', async () => {
		render(DenPane, {
			data: openPage(untitled, { body: 'listing', entries: [], more: undefined })
		});
		await expect.element(page.getByTestId('den-empty')).toHaveTextContent('This folder is empty.');
	});

	it('says "Content not shown yet." for a flagged folder, whatever its listing', async () => {
		const posts = open('/den/posts', 'posts', { contentNotShown: true, ownLevel: 'public' });
		render(DenPane, { data: openPage(posts, { body: 'listing', entries: [], more: undefined }) });
		await expect
			.element(page.getByTestId('den-not-shown'))
			.toHaveTextContent('Content not shown yet.');
	});

	it('says "Content not shown yet." for a file and for a content case this build doesn’t know', async () => {
		const changelog = open('/den/c/changelog', 'changelog', { kind: 'file' });
		render(DenPane, { data: openPage(changelog, { body: 'file' }) });
		await expect.element(page.getByTestId('den-not-shown')).toBeInTheDocument();
		await expect.element(page.getByTestId('node-kind')).toHaveTextContent('File');
	});

	it('says links don’t open yet for a link', async () => {
		render(DenPane, {
			data: openPage(open('/den/l', 'link', { kind: 'symlink' }), { body: 'link' })
		});
		await expect
			.element(page.getByTestId('den-link'))
			.toHaveTextContent("Opening links isn't available yet.");
	});

	it('shows a card with its name and "Can’t open", and never a visibility level', async () => {
		const card = data({
			outcome: 'page',
			page: {
				view: 'card',
				node: {
					view: 'card',
					name: denName('Some commission'),
					type: denType('commission'),
					mounted: true
				},
				crumbs: []
			},
			flagLinks: { on: denHref('/den/x'), off: denHref('/den/x') }
		});
		render(DenPane, { data: card });

		await expect
			.element(page.getByRole('heading', { name: 'Some commission' }))
			.toBeInTheDocument();
		await expect.element(page.getByTestId('node-kind')).toHaveTextContent("Can't open");
		await expect.element(page.getByTestId('den-card')).toBeInTheDocument();
		await expect.element(page.getByTestId('visibility')).not.toBeInTheDocument();
	});

	it('shows a card row in a listing as a row that isn’t a link', async () => {
		const card: DenEntry = {
			view: 'card',
			name: denName('Some commission'),
			type: denType('commission'),
			mounted: true
		};
		render(DenPane, {
			data: openPage(untitled, { body: 'listing', entries: [card], more: undefined })
		});

		const row = page.getByTestId('entry-card');
		await expect.element(row).toHaveTextContent('Some commission');
		expect(row.element().closest('a')).toBeNull();
	});

	it('shows the one not-found, with a way back to My Den', async () => {
		render(DenPane, { data: data({ outcome: 'notFound' }) });
		await expect.element(page.getByRole('heading', { name: 'Nothing here' })).toBeInTheDocument();
		await expect
			.element(page.getByTestId('den-not-found'))
			.toHaveTextContent("This doesn't exist, or you can't open it.");
		await expect
			.element(page.getByRole('link', { name: 'Back to My Den' }))
			.toHaveAttribute('href', '/den');
	});

	it('shows "not connected yet" on a live run', async () => {
		render(DenPane, { data: data({ outcome: 'notConnected' }) });
		await expect
			.element(page.getByTestId('den-not-connected'))
			.toHaveTextContent("My Den isn't connected to the backend yet.");
	});

	it('renders an API problem with the one problem note', async () => {
		const problem = {
			type: 'urn:zurfur:error:rate-limited',
			code: 'rate_limited',
			title: 'Slow',
			detail: 'Slow down.',
			status: 429
		};
		render(DenPane, { data: data({ outcome: 'problem', problem }) });
		await expect.element(page.getByTestId('problem')).toHaveTextContent('Slow down.');
	});

	it('swaps the entries for skeleton rows and marks itself busy while the next item loads', async () => {
		render(DenPane, {
			data: openPage(untitled, {
				body: 'listing',
				entries: [open('/den/a', 'a')],
				more: undefined
			}),
			busy: true
		});
		await expect.element(page.getByTestId('den-pane')).toHaveAttribute('aria-busy', 'true');
		await expect.element(page.getByTestId('skeleton')).toBeInTheDocument();
		await expect.element(page.getByTestId('entry-list')).not.toBeInTheDocument();
	});

	it('appends pages fetched in place, and drops More after the last', async () => {
		const first = {
			body: 'listing' as const,
			entries: [open('/den/a', 'a')],
			more: denHref('/den?pageToken=p1.1')
		};
		render(DenPane, {
			data: openPage(untitled, first),
			continuation: { entries: [open('/den/b', 'b')], more: undefined }
		});
		const rows = page.getByTestId('entry-list').element().querySelectorAll('li');
		expect(rows).toHaveLength(2);
		await expect.element(page.getByTestId('entry-more')).not.toBeInTheDocument();
	});

	it('names are text, never markup', async () => {
		const sneaky = open('/den/s', '<img src=x onerror=alert(1)>');
		render(DenPane, {
			data: openPage(
				open('/den', 'Alice'),
				{ body: 'listing', entries: [sneaky], more: undefined },
				true
			)
		});
		await expect.element(page.getByText('<img src=x onerror=alert(1)>')).toBeInTheDocument();
		expect(page.getByTestId('entry-list').element().querySelector('img')).toBeNull();
	});
});
