import { describe, expect, it } from 'vitest';
import * as published from '@zurfur/ui';
import * as source from '../lib/index';

describe('the public entry', () => {
	it('resolves @zurfur/ui through the svelte condition to src/lib/index.ts', () => {
		expect(published).toBe(source);
	});

	it('exports what src/lib/index.ts exports, which is nothing yet', () => {
		expect(Object.keys(published)).toEqual(Object.keys(source));
		expect(Object.keys(published)).toEqual([]);
	});
});
