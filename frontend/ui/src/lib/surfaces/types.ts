/** The plain shapes the surfaces and feedback pieces take. */
import type { LinkHref } from '../controls/link-href';
import type { IconName } from '../icons/icons';

/** An action a feedback piece offers, drawn as a Button: a press it reports, or a link it follows. */
export type FeedbackAction =
	| {
			readonly kind: 'press';
			readonly label: string;
			readonly icon?: IconName;
			readonly href?: never;
	  }
	| {
			readonly kind: 'link';
			readonly label: string;
			readonly icon?: IconName;
			readonly href: LinkHref;
	  };

/** The elements a Card may be: a plain box, a section, an article, or a list item. */
export type CardElement = 'div' | 'section' | 'article' | 'li';

/** A Card's padding: `none` is for a card that holds a picture edge to edge. */
export type CardPadding = 'default' | 'none';

/** A Badge's tone: the design system's six. */
export type BadgeTone = 'neutral' | 'accent' | 'iris' | 'crown' | 'rose' | 'taupe';

/** A status part's tone. There is no danger tone, by type. */
export type StatusTone = 'neutral' | 'progress' | 'waiting' | 'done';

/** One part of a status pill: its label, its tone and an optional icon. */
export interface StatusPart {
	readonly label: string;
	readonly tone: StatusTone;
	readonly icon?: IconName;
}

/** A status pill's parts: one, or two side by side. */
export type StatusParts = readonly [StatusPart] | readonly [StatusPart, StatusPart];

/** The quiet line beside a status pill. */
export interface StatusFact {
	readonly text: string;
	readonly icon?: IconName;
}

/** Who can see a thing. */
export type VisibilityLevel = 'private' | 'listed' | 'public';

/** The word shown for each visibility level, so a word and its icon can't disagree. */
export interface VisibilityWords {
	readonly private: string;
	readonly listed: string;
	readonly public: string;
}

/** An Avatar's size. */
export type AvatarSize = 'sm' | 'md' | 'lg';

/** An Avatar's tone: the same six as a Badge. */
export type AvatarTone = BadgeTone;
