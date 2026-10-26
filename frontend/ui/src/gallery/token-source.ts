import tokens from '@zurfur/ui/tokens.json';
import additions from '@zurfur/ui/tokens.additions.json';
import { parseTokenModel, type ParseOutcome, type Token, type TokenModel } from '@zurfur/ui';

/** The token files parsed once: the model every token page reads, or the reasons it was refused. */
export const tokenOutcome: ParseOutcome = parseTokenModel(tokens, additions);

/** The tokens of one group of the model, `sound` included; none when no group has the id. */
export function tokensOf(model: TokenModel, groupId: string): readonly Token[] {
	if (groupId === model.sound.id) return model.sound.tokens;
	return model.groups.find((group) => group.id === groupId)?.tokens ?? [];
}
