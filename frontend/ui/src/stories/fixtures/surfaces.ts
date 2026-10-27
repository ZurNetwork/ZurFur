import {
	isImageSrc,
	pinPointOf,
	type AvatarSize,
	type BadgeTone,
	type ImageSrc,
	type PinPoint,
	type StatusFact,
	type StatusParts,
	type VisibilityWords
} from '@zurfur/ui';

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

/** The fixture picture, checked by the picture guard; undefined if it ever stops passing. */
const avatarAddress = '/fixtures/avatar.svg';
export const avatarPicture: ImageSrc | undefined = isImageSrc(avatarAddress)
	? avatarAddress
	: undefined;

/** The names and handle the avatars show, the odd ones included. */
export const avatarWords = {
	name: 'Mika',
	handle: 'mika.test',
	marks: 'Z\u0351\u0352\u0353\u0354\u0355\u0356\u0357\u0358\u0359oe',
	emoji: '\u{1F9D1}\u{1F3FD}\u200D\u{1F3A8} Sol',
	noLetters: '...',
	hostile: '<img src=x onerror=boom()>'
} as const;

/** The name as heard, without the isolates the Avatar wraps it in, to find the circle by its name. */
export function withoutIsolates(text: string): string {
	return text.replace(/[\u2068\u2069]/gu, '');
}

/** The three avatar sizes, in the order the stories show them. */
export const avatarSizes: readonly AvatarSize[] = ['sm', 'md', 'lg'];

/** What the placeholder art is called for screen readers. */
export const placeholderArtWords = { label: 'Placeholder art' } as const;

/** One pin on the art: where it sits, the mark in its circle, and its name. */
export interface ArtPin {
	readonly point: PinPoint;
	readonly mark: string;
	readonly label: string;
}

/** A pin at (`x`, `y`), or none if the point ever stops passing. */
function pinAt(x: number, y: number, mark: string, label: string): readonly ArtPin[] {
	const point = pinPointOf(x, y);
	return point === undefined ? [] : [{ point, mark, label }];
}

/** Each pin's name, with its number in it. */
export const pinLabels = {
	ear: 'Pin 1: Left ear',
	muzzle: 'Pin 2: Muzzle',
	tail: 'Pin 3: Tail tip'
} as const;

/** The pins on the art. */
export const artPins: readonly ArtPin[] = [
	...pinAt(0.3, 0.3, '1', pinLabels.ear),
	...pinAt(0.62, 0.48, '2', pinLabels.muzzle),
	...pinAt(0.45, 0.8, '3', pinLabels.tail)
];

/** The one pin the single-pin stories show. */
export const onePin: readonly ArtPin[] = artPins.slice(0, 1);
