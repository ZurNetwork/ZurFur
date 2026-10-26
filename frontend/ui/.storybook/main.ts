/**
 * Storybook's main config for the component gallery: Svelte stories and MDX
 * docs under src/stories, the four addons, nothing reported, no remote
 * Storybooks composed in, and the server channel stripped of every handler
 * that fetches, writes files, starts programs or reports usage.
 */
import type { StorybookConfig } from '@storybook/sveltekit';
import type { Channel } from 'storybook/internal/channels';
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
	TELEMETRY_ERROR,
	TOGGLE_WHATS_NEW_NOTIFICATIONS
} from 'storybook/internal/core-events';
import { REVIEW_EVENTS } from 'storybook/internal/types';

/** The calls the hook makes on Storybook's server channel. */
export type ServerChannel = Pick<Channel, 'on' | 'emit' | 'removeAllListeners'>;

/** Why a dropped request failed, in the words the manager shows. */
const switchedOff = 'Switched off in this gallery.';

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** An own field of a request payload, or undefined. */
function fieldOf(payload: unknown, key: string): unknown {
	return isFields(payload) && Object.hasOwn(payload, key) ? payload[key] : undefined;
}

/** A request's `id`, as the core's request-response pairs carry it, or `''`. */
function requestId(payload: unknown): string {
	const id = fieldOf(payload, 'id');
	return typeof id === 'string' ? id : '';
}

/** One dropped request the manager waits on, and the failure answer that ends the wait. */
interface Answered {
	readonly request: string;
	readonly answer: (payload: unknown) => readonly [event: string, ...args: unknown[]];
}

/**
 * The requests the manager waits on. Each is answered the way the core
 * answers when the work fails, so the manager stops waiting and shows the
 * failure; nothing is fetched, written or started.
 */
const answered: readonly Answered[] = [
	{
		request: REQUEST_WHATS_NEW_DATA,
		answer: () => [RESULT_WHATS_NEW_DATA, { data: { status: 'ERROR' } }]
	},
	{
		request: SAVE_STORY_REQUEST,
		answer: (payload) => [
			SAVE_STORY_RESPONSE,
			{ id: requestId(payload), success: false, error: switchedOff }
		]
	},
	{
		request: CREATE_NEW_STORYFILE_REQUEST,
		answer: (payload) => [
			CREATE_NEW_STORYFILE_RESPONSE,
			{ id: requestId(payload), success: false, error: switchedOff }
		]
	},
	{
		request: FILE_COMPONENT_SEARCH_REQUEST,
		answer: (payload) => [
			FILE_COMPONENT_SEARCH_RESPONSE,
			{ id: requestId(payload), success: false, error: switchedOff }
		]
	},
	{
		// The manager matches the answer to its request by file, line and column.
		request: OPEN_IN_EDITOR_REQUEST,
		answer: (payload) => [
			OPEN_IN_EDITOR_RESPONSE,
			{
				file: fieldOf(payload, 'file'),
				line: fieldOf(payload, 'line'),
				column: fieldOf(payload, 'column'),
				error: switchedOff
			}
		]
	},
	{ request: GHOST_STORIES_REQUEST, answer: () => [GHOST_STORIES_RESPONSE] },
	{ request: AI_SETUP_ANALYTICS_REQUEST, answer: () => [AI_SETUP_ANALYTICS_RESPONSE] }
];

/**
 * Events whose core handlers write files or report usage and that nobody
 * waits on: dropped with no answer.
 */
export const silenced: readonly string[] = [
	SET_WHATS_NEW_CACHE,
	TOGGLE_WHATS_NEW_NOTIFICATIONS,
	TELEMETRY_ERROR,
	PREVIEW_INITIALIZED,
	SHARE_ISOLATE_MODE,
	SIDEBAR_FILTER_CHANGED,
	AI_PROMPT_NUDGE,
	REVIEW_EVENTS.PAGEVIEW
];

/** Every event whose core handlers the hook removes. */
export const dropped: readonly string[] = [...answered.map((entry) => entry.request), ...silenced];

/**
 * Runs after the core's and the addons' server-channel hooks: removes every
 * handler for the dropped events, answers the requests the manager waits on
 * with their failure, and hands the channel on.
 */
function serverChannel<Server extends ServerChannel>(channel: Server): Server {
	for (const event of dropped) channel.removeAllListeners(event);
	for (const { request, answer } of answered) {
		channel.on(request, (payload: unknown) => {
			const [event, ...args] = answer(payload);
			channel.emit(event, ...args);
		});
	}
	return channel;
}

/** Storybook's config, with the one server-channel hook its types don't name. */
type GalleryConfig = StorybookConfig & {
	readonly experimental_serverChannel: typeof serverChannel;
};

const config: GalleryConfig = {
	framework: '@storybook/sveltekit',
	stories: ['../src/stories/**/*.mdx', '../src/stories/**/*.stories.svelte'],
	addons: [
		'@storybook/addon-svelte-csf',
		'@storybook/addon-docs',
		'@storybook/addon-a11y',
		'@storybook/addon-vitest'
	],
	core: {
		disableTelemetry: true,
		enableCrashReports: false,
		disableWhatsNewNotifications: true,
		disableProjectJson: true
	},
	// A function replaces the composed Storybooks outright; an object would be
	// merged with the ones Storybook finds in the dependencies on its own.
	refs: () => ({}),
	experimental_serverChannel: serverChannel
};

export default config;
