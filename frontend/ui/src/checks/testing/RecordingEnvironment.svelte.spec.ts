import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import RecordingEnvironment from './RecordingEnvironment.svelte';
import Toggle from '../../lib/controls/Toggle.svelte';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

describe('RecordingEnvironment', () => {
	it('records each attachment once, however often the control below re-renders', async () => {
		const calls: SeamCall[] = [];
		const screen = render(
			Toggle,
			{ label: 'Show my queue', checked: false, onCheckedChange: () => undefined },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		await screen.rerender({ checked: true });
		await screen.rerender({ checked: false });
		const squishes = calls.filter((call) => call.kind === 'move' && call.move === 'squish');
		expect(squishes).toHaveLength(1);
	});
});
