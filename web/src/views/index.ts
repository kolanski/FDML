import { architectureBlock } from './architectureBlock';
import { specBlock } from './specBlock';
import { graphBlock } from './graphBlock';
import { flowsBlock } from './flowsBlock';
import type { ViewBlock } from './types';

export type { ViewBlock, ViewContext } from './types';

/**
 * The view-block registry. Order here = tab order.
 *
 * To add a view (e.g. a C4 block): create `views/c4Block.tsx` exporting a `ViewBlock`,
 * then add it to this array. App.tsx and Toolbar.tsx need no changes — they render
 * whatever is registered. To remove/swap a view: edit only this line.
 */
export const VIEW_BLOCKS: ViewBlock[] = [architectureBlock, specBlock, graphBlock, flowsBlock];
