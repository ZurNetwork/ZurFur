/*
 * The server-channel hook in .storybook/main.ts, on Storybook's own Channel
 * class with a recording transport: the core's handlers for the dropped
 * events are gone (stand-ins play them here), the requests the manager waits
 * on get their failure answer and nothing is fetched, and the handlers the
 * gallery needs (the test panel's among them) stay.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Channel } from 'storybook/internal/channels';
import {
	AI_PROMPT_NUDGE,
	AI_SETUP_ANALYTICS_REQUEST,
	AI_SETUP_ANALYTICS_RESPONSE,
	CREATE_NEW_STORYFILE_REQUEST,
	CREATE_NEW_STORYFILE_RESPONSE,
	FILE_COMPONENT_SEARCH_REQUEST,
	FILE_COMPONENT_SEARCH_RESPONSE,
	GHOST_STORIES_REQUEST,
	GHOST_STORIES_RESPONSE,
	OPEN_IN_EDITOR_REQUEST,
	OPEN_IN_EDITOR_RESPONSE,
	PREVIEW_INITIALIZED,
	REQUEST_WHATS_NEW_DATA,
	RESULT_WHATS_NEW_DATA,
	SAVE_STORY_REQUEST,
	SAVE_STORY_RESPONSE,
	SET_WHATS_NEW_CACHE,
	SHARE_ISOLATE_MODE,
	SIDEBAR_FILTER_CHANGED,
	STORY_INDEX_INVALIDATED,
	TELEMETRY_ERROR,
	TOGGLE_WHATS_NEW_NOTIFICATIONS
} from 'storybook/internal/core-events';
import { REVIEW_EVENTS } from 'storybook/internal/types';
import {
	STORE_CHANNEL_EVENT_NAME,
	STORYBOOK_ADDON_TEST_CHANNEL,
	TRIGGER_TEST_RUN_REQUEST
} from '@storybook/addon-vitest/constants';
import config from '../../.storybook/main.ts';

interface SentEvent {
	readonly type: string;
	readonly args: readonly unknown[];
}

/** A channel like the dev server's, whose transport records what it sends and delivers what a page sends. */
function recordingChannel() {
	const sent: SentEvent[] = [];
	let deliver: (event: SentEvent) => void = () => {
		throw new Error('the channel never took the transport');
	};
	const channel = new Channel({
		transports: [
			{
				setHandler: (handler) => {
					deliver = (event) => {
						handler({ ...event, args: [...event.args], from: 'manager' });
					};
				},
				send: (event: SentEvent) => {
					sent.push({ type: event.type, args: event.args });
				}
			}
		]
	});
	const fromPage = (type: string, ...args: unknown[]): void => {
		deliver({ type, args });
	};
	return { channel, sent, fromPage };
}

const switchedOff = 'Switched off in this gallery.';

/** The requests the manager waits on, a sample of what it sends, and the answer that ends the wait. */
const answered: readonly (readonly [string, unknown, string, unknown])[] = [
	[REQUEST_WHATS_NEW_DATA, undefined, RESULT_WHATS_NEW_DATA, { data: { status: 'ERROR' } }],
	[
		SAVE_STORY_REQUEST,
		{ id: 'save-1', payload: { csfId: 'a--b', importPath: './x.stories.svelte', args: '{}' } },
		SAVE_STORY_RESPONSE,
		{ id: 'save-1', success: false, error: switchedOff }
	],
	[
		CREATE_NEW_STORYFILE_REQUEST,
		{ id: 'create-1', payload: { componentFilePath: 'src/x.svelte' } },
		CREATE_NEW_STORYFILE_RESPONSE,
		{ id: 'create-1', success: false, error: switchedOff }
	],
	[
		FILE_COMPONENT_SEARCH_REQUEST,
		{ id: 'Button', payload: {} },
		FILE_COMPONENT_SEARCH_RESPONSE,
		{ id: 'Button', success: false, error: switchedOff }
	],
	[
		OPEN_IN_EDITOR_REQUEST,
		{ file: 'src/x.svelte', line: 3, column: 7 },
		OPEN_IN_EDITOR_RESPONSE,
		{ file: 'src/x.svelte', line: 3, column: 7, error: switchedOff }
	],
	[GHOST_STORIES_REQUEST, undefined, GHOST_STORIES_RESPONSE, undefined],
	[AI_SETUP_ANALYTICS_REQUEST, undefined, AI_SETUP_ANALYTICS_RESPONSE, undefined]
];

/** Events whose core handlers write files or report usage, dropped with no answer. */
const silenced: readonly string[] = [
	SET_WHATS_NEW_CACHE,
	TOGGLE_WHATS_NEW_NOTIFICATIONS,
	TELEMETRY_ERROR,
	PREVIEW_INITIALIZED,
	SHARE_ISOLATE_MODE,
	SIDEBAR_FILTER_CHANGED,
	AI_PROMPT_NUDGE,
	REVIEW_EVENTS.PAGEVIEW
];

/** Events the gallery needs: the story index, and addon-vitest's test runs and its store. */
const kept: readonly string[] = [
	STORY_INDEX_INVALIDATED,
	TRIGGER_TEST_RUN_REQUEST,
	STORYBOOK_ADDON_TEST_CHANNEL,
	STORE_CHANNEL_EVENT_NAME
];

const fetchSpy = vi.fn();

beforeEach(() => {
	vi.stubGlobal('fetch', fetchSpy);
});

afterEach(() => {
	vi.unstubAllGlobals();
	fetchSpy.mockReset();
});

/**
 * A channel loaded the way the core and the addons leave it: a stand-in
 * handler on every dropped and every kept event (the what's-new one fetches,
 * as the core's does), then the hook.
 */
function hookedChannel() {
	const recording = recordingChannel();
	const standIns = new Map<string, () => void>();
	for (const [request] of answered) standIns.set(request, vi.fn());
	standIns.set(
		REQUEST_WHATS_NEW_DATA,
		vi.fn(() => {
			fetchSpy('whats-new');
		})
	);
	for (const event of [...silenced, ...kept]) standIns.set(event, vi.fn());
	for (const [event, standIn] of standIns) recording.channel.on(event, standIn);
	const returned = config.experimental_serverChannel(recording.channel);
	return { ...recording, standIns, returned };
}

describe("the server channel's hook", () => {
	it('hands the same channel on, as the preset chain expects', () => {
		const { channel, returned } = hookedChannel();
		expect(returned).toBe(channel);
	});

	it.each(answered.map(([request]) => request))('leaves only its own answer on %s', (request) => {
		const { channel, standIns } = hookedChannel();
		const listeners = channel.listeners(request) ?? [];
		expect(listeners).not.toContain(standIns.get(request));
		expect(listeners).toHaveLength(1);
	});

	it.each(silenced)('leaves no handler on %s', (event) => {
		const { channel } = hookedChannel();
		expect(channel.listenerCount(event)).toBe(0);
	});

	it.each(kept)('keeps the handler on %s', (event) => {
		const { channel, standIns, fromPage } = hookedChannel();
		expect(channel.listeners(event)).toEqual([standIns.get(event)]);
		fromPage(event, { type: 'probe' });
		expect(standIns.get(event)).toHaveBeenCalledOnce();
	});

	it("answers what's new with the core's error shape and fetches nothing", () => {
		const { sent, fromPage } = hookedChannel();
		fromPage(REQUEST_WHATS_NEW_DATA);
		expect(sent).toEqual([{ type: RESULT_WHATS_NEW_DATA, args: [{ data: { status: 'ERROR' } }] }]);
		expect(fetchSpy).not.toHaveBeenCalled();
	});

	it.each(answered)(
		'answers %s with its failure, and runs no stand-in',
		(request, payload, response, answer) => {
			const { sent, fromPage, standIns } = hookedChannel();
			fromPage(request, payload);
			const expectedArgs = answer === undefined ? [] : [answer];
			expect(sent).toEqual([{ type: response, args: expectedArgs }]);
			expect(standIns.get(request)).not.toHaveBeenCalled();
		}
	);

	it.each([
		['no payload', undefined],
		['a payload that is text', 'x'],
		['a payload whose id is a number', { id: 7 }]
	])('answers a save with an empty id for %s', (_label, payload) => {
		const { sent, fromPage } = hookedChannel();
		fromPage(SAVE_STORY_REQUEST, payload);
		expect(sent).toEqual([
			{ type: SAVE_STORY_RESPONSE, args: [{ id: '', success: false, error: switchedOff }] }
		]);
	});

	it('answers open-in-editor with its own error, whatever error the request carried', () => {
		const { sent, fromPage } = hookedChannel();
		fromPage(OPEN_IN_EDITOR_REQUEST, { file: 'x', error: null });
		expect(sent).toEqual([
			{
				type: OPEN_IN_EDITOR_RESPONSE,
				args: [{ file: 'x', line: undefined, column: undefined, error: switchedOff }]
			}
		]);
	});
});
