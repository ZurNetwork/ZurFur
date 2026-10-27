/** A language tag the platform accepts, and the one way to mint it. */

declare const localeTagBrand: unique symbol;

/** A BCP 47 language tag, such as `en-GB`, that the platform's `Intl` accepts. */
export type LocaleTag = string & { readonly [localeTagBrand]: true };

/** Whether `text` is one language tag the platform accepts; never throws, whatever the text. */
export function isLocaleTag(text: string): text is LocaleTag {
	if (text === '') return false;
	try {
		return Intl.getCanonicalLocales(text).length === 1;
	} catch {
		return false;
	}
}
