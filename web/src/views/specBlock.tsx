import SpecView from '../components/SpecView';
import type { ViewBlock } from './types';

export const specBlock: ViewBlock = {
  id: 'spec',
  label: 'Spec',
  render: (ctx) => <SpecView spec={ctx.spec} searchQuery={ctx.searchQuery} hierarchy={ctx.hierarchy} />,
};
