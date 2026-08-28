import Canvas from '../components/Canvas';
import type { ViewBlock } from './types';

export const graphBlock: ViewBlock = {
  id: 'graph',
  label: 'Graph',
  render: (ctx) => <Canvas initialNodes={ctx.nodes} initialEdges={ctx.edges} />,
};
