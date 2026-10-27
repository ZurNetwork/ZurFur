/** Hands the preview's annotations, the accessibility addon's first, to every story test. */
import * as a11yAddonAnnotations from '@storybook/addon-a11y/preview';
import { setProjectAnnotations } from '@storybook/sveltekit';
import * as projectAnnotations from './preview.ts';

setProjectAnnotations([a11yAddonAnnotations, projectAnnotations]);
