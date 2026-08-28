import FlowsView from '../components/FlowsView';
import type { ViewBlock } from './types';

export const flowsBlock: ViewBlock = {
  id: 'flows',
  label: 'Flows',
  render: (ctx) => <FlowsView spec={ctx.spec} inferredFlows={ctx.hierarchy.inferredFlows} />,
};
