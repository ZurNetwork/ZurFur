import { describe, expect, it } from 'vitest';
import * as published from '@zurfur/ui';
import * as source from '../lib/index';

describe('the public entry', () => {
	it('resolves @zurfur/ui through the svelte condition to src/lib/index.ts', () => {
		expect(published).toBe(source);
	});

	it('exports exactly the paper lists, the environment names, the token model, the icons, the first controls, the first surfaces, the link, day, locale, picture and pin mints and the motion seam', () => {
		expect(Object.keys(published).sort()).toEqual([
			'Avatar',
			'Badge',
			'Button',
			'Card',
			'Checkbox',
			'DateField',
			'ICON_NAMES',
			'Icon',
			'IconButton',
			'MOTION_ATTRIBUTE',
			'Menu',
			'NO_MOVES',
			'NO_SOUNDS',
			'PAPER_CLASSES',
			'PAPER_MOVES',
			'PillSwitch',
			'Pin',
			'SKIN_ATTRIBUTE',
			'STILL_ENVIRONMENT',
			'ScrollingTabs',
			'Select',
			'Skeleton',
			'Spinner',
			'StatusPill',
			'Tabs',
			'TextInput',
			'ToastStack',
			'Toggle',
			'VisibilityMarker',
			'findToken',
			'isCalendarDay',
			'isIconName',
			'isImageSrc',
			'isLinkHref',
			'isLocaleTag',
			'isSameOriginPath',
			'isTokenName',
			'parseSkin',
			'parseTokenModel',
			'pinPointOf',
			'provideEnvironment',
			'resolveToken',
			'valueIn'
		]);
	});
});
