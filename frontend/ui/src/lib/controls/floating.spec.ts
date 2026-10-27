import { describe, expect, it } from 'vitest';
import { portalTarget } from './floating.ts';

/** A stand-in page: just the parts `portalTarget` reads. */
interface FakePage {
	readonly documentElement: object;
	readonly body: object;
}

/** A stand-in anchor whose nearest match for the skin selector is `scope`, recording what it was asked. */
function anchorIn(page: FakePage, scope: object | undefined, asked: string[]): Element {
	const anchor = {
		ownerDocument: page,
		closest: (selectors: string) => {
			asked.push(selectors);
			return scope ?? null;
		}
	};
	return anchor as unknown as Element;
}

function freshPage(): FakePage {
	return { documentElement: { tag: 'html' }, body: { tag: 'body' } };
}

describe('portalTarget', () => {
	it('portals into the nearest skin scope around the anchor', () => {
		const page = freshPage();
		const lightCopy = { tag: 'div' };
		const asked: string[] = [];
		expect(portalTarget(anchorIn(page, lightCopy, asked))).toBe(lightCopy);
		expect(asked).toEqual(['[data-skin]']);
	});

	it('portals into the body when the nearest skin scope is the page itself', () => {
		const page = freshPage();
		expect(portalTarget(anchorIn(page, page.documentElement, []))).toBe(page.body);
	});

	it('portals into the body when nothing sets a skin', () => {
		const page = freshPage();
		expect(portalTarget(anchorIn(page, undefined, []))).toBe(page.body);
	});
});
