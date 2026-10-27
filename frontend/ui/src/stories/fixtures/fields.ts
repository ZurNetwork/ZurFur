/** The words, options and days the field-like controls' stories share. */
import {
	isCalendarDay,
	isLocaleTag,
	type CalendarDay,
	type DateFieldWords,
	type LocaleTag,
	type MenuItem,
	type MenuTrigger,
	type SelectOption,
	type TabItem
} from '@zurfur/ui';

/** The made-up artist the handle and DID samples belong to. */
export const ARTIST_HANDLE = 'artist.test';
export const ARTIST_DID = 'did:plc:abcdefghijklmnopqrstuvwx';

/** Words for the text input's states. */
export const TEXT_WORDS = {
	title: 'Commission title',
	titleExample: 'A dragon asleep on a pile of books',
	hint: 'Shown to the artist in the first message',
	error: 'That handle isn’t on Bluesky. Check the spelling?',
	handle: 'Artist handle',
	did: 'Artist DID',
	search: 'Search commissions',
	notes: 'Notes for the artist',
	notesExample: 'Soft light from the left.\nWarm colours, no hard outlines.'
} as const;

/** A commission's stages, as the select's values. */
export type Stage = 'sketch' | 'lineart' | 'colour' | 'final';

/** The select's options. */
export const STAGES: readonly SelectOption<Stage>[] = [
	{ value: 'sketch', label: 'Sketch' },
	{ value: 'lineart', label: 'Lineart' },
	{ value: 'colour', label: 'Colour' },
	{ value: 'final', label: 'Final' }
];

/** Words for the select's states. */
export const SELECT_WORDS = {
	label: 'Stage',
	placeholder: 'Choose a stage',
	hint: 'You can change this later',
	error: 'Pick a stage to go on'
} as const;

/** The sections of a profile, as the tabs' ids. */
export type ProfileSection = 'gallery' | 'comms' | 'ocs';

/** Tabs with an icon and a count, a count alone, and neither. */
export const PROFILE_TABS: readonly TabItem<ProfileSection>[] = [
	{ id: 'gallery', label: 'Gallery', icon: 'image', count: '24' },
	{ id: 'comms', label: 'Comms', count: '2' },
	{ id: 'ocs', label: 'OCs' }
];

/** The same sections, plain. */
export const PLAIN_TABS: readonly TabItem<ProfileSection>[] = [
	{ id: 'gallery', label: 'Gallery' },
	{ id: 'comms', label: 'Comms' },
	{ id: 'ocs', label: 'OCs' }
];

/** More sections, as the many-tabs story's ids. */
export type LongSection =
	'gallery' | 'comms' | 'ocs' | 'collections' | 'posts' | 'reviews' | 'friends' | 'about';

/** Enough tabs to outgrow a narrow screen. */
export const LONG_TABS: readonly TabItem<LongSection>[] = [
	{ id: 'gallery', label: 'Gallery', icon: 'image', count: '24' },
	{ id: 'comms', label: 'Comms', count: '2' },
	{ id: 'ocs', label: 'OCs', count: '6' },
	{ id: 'collections', label: 'Collections' },
	{ id: 'posts', label: 'Posts' },
	{ id: 'reviews', label: 'Reviews', count: '18' },
	{ id: 'friends', label: 'Friends' },
	{ id: 'about', label: 'About' }
];

/** The tab list's accessible name. */
export const TABS_LABEL = 'Profile';

/** What a commission's menu can do, as the menu's ids. */
export type CommissionAction = 'rename' | 'duplicate' | 'archive' | 'delete';

/** The menu's rows: with an icon, disabled, and one that can't be undone. */
export const COMMISSION_ACTIONS: readonly MenuItem<CommissionAction>[] = [
	{ id: 'rename', label: 'Rename', icon: 'file' },
	{ id: 'duplicate', label: 'Duplicate', icon: 'copy' },
	{ id: 'archive', label: 'Move to archive', icon: 'archive', disabled: true },
	{ id: 'delete', label: 'Delete commission', icon: 'trash-2', tone: 'danger' }
];

/** A trigger with words. */
export const WORDS_TRIGGER: MenuTrigger = { kind: 'button', label: 'Actions' };

/** A trigger with words and an icon. */
export const ICON_WORDS_TRIGGER: MenuTrigger = { kind: 'button', label: 'Actions', icon: 'list' };

/** A trigger that is an icon alone, named for a screen reader. */
export const ICON_TRIGGER: MenuTrigger = { kind: 'icon', label: 'More actions', icon: 'ellipsis' };

/** The date field's words: each empty segment shows its own, and a screen reader hears "empty". */
export const DATE_WORDS: DateFieldWords = {
	day: 'day',
	month: 'month',
	year: 'year',
	empty: 'empty'
};

/** Words for the date field's states. */
export const DATE_TEXT = {
	label: 'Due date',
	hint: 'The day you would like it by',
	error: 'Pick a day from the first of January'
} as const;

/** A locale prop when `text` is a language tag the platform accepts, else nothing. */
function localeProp(text: string): { readonly locale?: LocaleTag } {
	return isLocaleTag(text) ? { locale: text } : {};
}

/** Day, month, year: the order of Britain. */
export const DAY_FIRST = localeProp('en-GB');

/** Month, day, year: the order of America. */
export const MONTH_FIRST = localeProp('en-US');

/** A calendar day when `text` names one, else undefined. */
function dayOf(text: string): CalendarDay | undefined {
	return isCalendarDay(text) ? text : undefined;
}

/** The day a filled field shows. */
export const DUE_DAY = dayOf('2026-04-18');

/** The first day the limits story allows. */
export const EARLIEST_DAY = dayOf('2026-01-01');

/** The last day the limits story allows. */
export const LATEST_DAY = dayOf('2026-12-31');

/** The day an empty segment steps from. */
export const STEP_START_DAY = dayOf('2026-06-15');
