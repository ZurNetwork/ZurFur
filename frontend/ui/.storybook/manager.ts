/**
 * The manager's look: Storybook's own theme, light or dark as the OS asks at
 * load, with the gallery's title in place of the Storybook logo. Storybook
 * renders the title as HTML, so it is this constant and nothing else.
 */
import { addons } from 'storybook/manager-api';
import { getPreferredColorScheme } from 'storybook/theming';
import { create } from 'storybook/theming/create';

/** The brand title in the manager's sidebar. */
export const BRAND_TITLE = 'Zurfur Component Gallery';

addons.setConfig({
	theme: create({ base: getPreferredColorScheme(), brandTitle: BRAND_TITLE })
});
