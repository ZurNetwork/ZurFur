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
