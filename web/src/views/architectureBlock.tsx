import ArchitectureView from '../components/ArchitectureView';
import type { ViewBlock } from './types';

/** C4-ish platform/context view: systems + integrations. Drill into a system from here. */
export const architectureBlock: ViewBlock = {
  id: 'architecture',
  label: 'Architecture',
  available: (ctx) => ctx.platformSpec.systems.length > 0,
  render: (ctx) => <ArchitectureView spec={ctx.platformSpec} onDrillDown={ctx.onDrillDown} />,
};
