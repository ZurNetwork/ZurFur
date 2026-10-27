<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect } from 'storybook/test';
	import { ToastStack } from '@zurfur/ui';
	import { toastStackWords } from '../fixtures/surfaces.ts';

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/ToastStack',
		component: ToastStack,
		tags: ['autodocs', 'simple', 'layer', 'phone-320'],
		argTypes: {
			children: { table: { disable: true } }
		},
		parameters: {
			layout: 'fullscreen',
			docs: {
				description: {
					component: `A simple component: the place toasts sit, fixed at the bottom right, and full width on phones. It is a labelled region with a polite live region inside it that is always there, even with no toasts, so a toast put in later is announced. The page mounts it once.`
				}
			}
		}
	});
</script>

<Story
	name="Empty"
	args={{ label: toastStackWords.label }}
	play={async ({ canvas }) => {
		const region = canvas.getByRole('region', { name: toastStackWords.label });
		await expect(region).toBeInTheDocument();
		await expect(region.querySelector('[aria-live="polite"]')).not.toBeNull();
	}}
/>
