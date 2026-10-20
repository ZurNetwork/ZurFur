/** Which section of the signed-in app the open page belongs to; the frame marks its link current. */
export type FrameSection = 'den' | 'accounts' | 'report' | 'other';

/** The section a `(session)` route id belongs to. */
export function frameSectionOf(routeId: string | undefined): FrameSection {
	if (routeId?.startsWith('/(session)/den') === true) return 'den';
	if (routeId?.startsWith('/(session)/accounts') === true) return 'accounts';
	if (routeId === '/(session)/report-a-bug') return 'report';
	return 'other';
}
