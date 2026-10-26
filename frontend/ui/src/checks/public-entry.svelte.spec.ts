import { describe, expect, it } from 'vitest';
import * as published from '@zurfur/ui';
import * as source from '../lib/index';

describe('the public entry', () => {
	it('resolves @zurfur/ui through the svelte condition to src/lib/index.ts', () => {
		expect(published).toBe(source);
	});

	it('exports exactly the paper lists, the environment and the token model', () => {
		expect(Object.keys(published).sort()).toEqual([
			'MOTION_ATTRIBUTE',
			'PAPER_CLASSES',
			'PAPER_MOVES',
			'SKIN_ATTRIBUTE',
			'findToken',
			'isTokenName',
			'parseSkin',
			'parseTokenModel',
			'resolveToken',
			'valueIn'
		]);
	});
});
