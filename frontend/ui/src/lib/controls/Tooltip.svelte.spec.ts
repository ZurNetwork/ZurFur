import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import RecordingTrigger from '../../checks/testing/RecordingTrigger.svelte';

/**
 * The string keys bits-ui's tooltip trigger hands over, less the description,
 * plus the button type; `disabled` comes only when it is true.
 */
const HANDED_KEYS: readonly string[] = [
	'data-delay-duration',
	'data-disabled',
	'data-state',
	'data-tooltip-trigger',
	'id',
	'onblur',
	'onclick',
	'onfocus',
	'onpointerdown',
	'onpointerenter',
	'onpointerleave',
	'onpointermove',
	'onpointerup',
	'tabindex',
	'type'
];

/** Renders Tooltip with a trigger that records its props, and hands back the last record. */
function recorded(disabled = false): Readonly<Record<string, unknown>> {
	const records: Readonly<Record<string, unknown>>[] = [];
	const onRecord = (props: Readonly<Record<string, unknown>>): void => {
		records.push(props);
	};
	render(RecordingTrigger, { onRecord, disabled });
	const last = records.at(-1);
	if (last === undefined) throw new Error('the trigger was handed nothing');
	return last;
}

describe('Tooltip', () => {
	it('hands its trigger exactly the tooltip trigger’s keys and a button type', () => {
		const props = recorded();
		expect(Object.keys(props).sort()).toEqual(HANDED_KEYS);
		expect(props.type).toBe('button');
	});

	it('adds disabled, set, when it is switched off', () => {
		const props = recorded(true);
		expect(Object.keys(props).sort()).toEqual([...HANDED_KEYS, 'disabled'].sort());
		expect(props.disabled).toBe(true);
	});

	it('hands its trigger two attachments: its own anchor and bits-ui’s', () => {
		const symbols = Object.getOwnPropertySymbols(recorded());
		expect(symbols).toHaveLength(2);
	});

	it.each(['aria-describedby', 'href', 'formaction', 'child', 'children'])(
		'hands its trigger no %s',
		(key) => {
			expect(Object.hasOwn(recorded(), key)).toBe(false);
		}
	);
});
