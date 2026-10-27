/** A picture's address on this site, and the one way to mint it. */
import { isSameOriginPath, type SameOriginPath } from '../controls/link-href';

declare const imageSrcBrand: unique symbol;

/** A same-origin path a picture may load from; nothing else reaches an `<img>`. */
export type ImageSrc = SameOriginPath & { readonly [imageSrcBrand]: true };

/** Whether `text` is a picture address: exactly a same-origin path, so no scheme, host or bare root. */
export function isImageSrc(text: string): text is ImageSrc {
	return isSameOriginPath(text);
}
