import { describe, expect, it } from 'vitest';
import * as published from '@zurfur/ui';
import * as source from '../lib/index';

describe('the public entry', () => {
	it('resolves @zurfur/ui through the svelte condition to src/lib/index.ts', () => {
		expect(published).toBe(source);
	});

	it('exports exactly the paper lists, the environment names, the token model, the icons, the first controls, the link mints and the motion seam', () => {
		expect(Object.keys(published).sort()).toEqual([
			'Button',
			'Checkbox',
			'ICON_NAMES',
			'Icon',
			'IconButton',
			'MOTION_ATTRIBUTE',
			'Menu',
			'NO_MOVES',
			'NO_SOUNDS',
			'PAPER_CLASSES',
			'PAPER_MOVES',
			'SKIN_ATTRIBUTE',
			'STILL_ENVIRONMENT',
			'Select',
			'TextInput',
			'Toggle',
			'findToken',
			'isIconName',
			'isLinkHref',
			'isSameOriginPath',
			'isTokenName',
			'parseSkin',
			'parseTokenModel',
			'provideEnvironment',
			'resolveToken',
			'valueIn'
		]);
	});
});
