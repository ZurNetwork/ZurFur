import type { BadgeTone, StatusFact, StatusParts, VisibilityWords } from '@zurfur/ui';

/** What the cards hold: a title, a line under it, and the lines of a short list. */
export const cardWords = {
	title: 'Sona Ref Sheet',
	note: 'Front, side and back, flats done',
	article: 'Comm update',
	articleNote: 'Lineart is in, colours next',
	action: 'Open the ref'
} as const;

/** The cards of the list story, one per work in progress. */
export const cardList: readonly string[] = ['Lineart', 'Flats', 'Shading'];

/** The label each badge tone wears. */
export const badgeWords: Readonly<Record<BadgeTone, string>> = {
	neutral: 'Open for comms',
	accent: 'Featured',
	iris: 'Collab',
	crown: 'Top pick',
	rose: 'Favourite',
	taupe: 'Archived'
};

/** The six tones, in the order the stories show them. */
export const badgeTones: readonly BadgeTone[] = [
	'neutral',
	'accent',
	'iris',
	'crown',
	'rose',
	'taupe'
];

/** What the spinner says while it works. */
export const spinnerWords = { loading: 'Loading sketches' } as const;

/** What the toast stack is called. */
export const toastStackWords = { label: 'Notifications' } as const;

/** One word per visibility level. */
export const visibilityWords: VisibilityWords = {
	private: 'Only me',
	listed: 'By link',
	public: 'Everyone'
};

/** A pill with one part. */
export const onePart: StatusParts = [{ label: 'Sketching', tone: 'progress', icon: 'circle-dot' }];

/** A pill with two parts: the work, and who it waits on. */
export const twoParts: StatusParts = [
	{ label: 'Sketching', tone: 'progress', icon: 'circle-dot' },
	{ label: 'Waiting on Mika', tone: 'waiting', icon: 'hourglass' }
];

/** The quiet line beside a pill. */
export const lateFact: StatusFact = { text: 'Due last week', icon: 'clock' };
