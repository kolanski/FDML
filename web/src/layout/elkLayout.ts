import ELK, { type ElkNode, type ElkExtendedEdge } from 'elkjs/lib/elk.bundled.js';
import type { Node, Edge } from '@xyflow/react';
import type { SpecNodeData } from './specToGraph';

const elk = new ELK();

const LAYER_MAP: Record<string, number> = {
  feature: 0,
  flow: 1,
  action: 2,
  constraint: 2,
  entity: 3,
  system: 4,
};

const COLLAPSED_WIDTH = 220;
const COLLAPSED_HEIGHT = 80;
const EXPANDED_WIDTH = 340;

function estimateExpandedHeight(data: SpecNodeData): number {
  let h = 60; // header
  const spec = data.spec as Record<string, unknown>;

  if (data.nodeType === 'entity') {
    const fields = (spec.fields as unknown[]) || [];
    h += 28 + fields.length * 22;
    const rels = (spec.relationships as unknown[]) || [];
    if (rels.length) h += 28 + rels.length * 20;
  } else if (data.nodeType === 'action') {
    if (spec.input) h += 40;
    if (spec.output) h += 40;
    const pre = (spec.preconditions as unknown[]) || [];
    const post = (spec.postconditions as unknown[]) || [];
    if (pre.length) h += 24 + pre.length * 18;
    if (post.length) h += 24 + post.length * 18;
  } else if (data.nodeType === 'feature') {
    const scenarios = (spec.scenarios as unknown[]) || [];
    for (const s of scenarios) {
      const sc = s as { given?: string[]; when?: string[]; then?: string[] };
      h += 30 + ((sc.given?.length || 0) + (sc.when?.length || 0) + (sc.then?.length || 0)) * 18;
    }
  } else if (data.nodeType === 'constraint') {
    h += 60;
  } else if (data.nodeType === 'flow') {
    const steps = (spec.steps as unknown[]) || [];
    h += 28 + steps.length * 22;
  }

  return Math.max(h, COLLAPSED_HEIGHT);
}

export async function layoutGraph(
  nodes: Node[],
  edges: Edge[],
): Promise<Node[]> {
  if (nodes.length === 0) return nodes;

  const elkNodes: ElkNode[] = nodes.map((node) => {
    const data = node.data as unknown as SpecNodeData;
    const w = data.expanded ? EXPANDED_WIDTH : COLLAPSED_WIDTH;
    const h = data.expanded ? estimateExpandedHeight(data) : COLLAPSED_HEIGHT;
    const layer = LAYER_MAP[data.nodeType] ?? 2;
    return {
      id: node.id,
      width: w,
      height: h,
      layoutOptions: {
        'org.eclipse.elk.layered.layering.layerConstraint': '',
        'org.eclipse.elk.partitioning.partition': String(layer),
      },
    };
  });

  const elkEdges: ElkExtendedEdge[] = edges.map((edge) => ({
    id: edge.id,
    sources: [edge.source],
    targets: [edge.target],
  }));

  const graph: ElkNode = {
    id: 'root',
    children: elkNodes,
    edges: elkEdges,
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': 'DOWN',
      'elk.spacing.nodeNode': '60',
      'elk.layered.spacing.nodeNodeBetweenLayers': '80',
      'elk.partitioning.activate': 'true',
      'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
    },
  };

  const layouted = await elk.layout(graph);

  return nodes.map((node) => {
    const elkNode = layouted.children?.find((n) => n.id === node.id);
    return {
      ...node,
      position: {
        x: elkNode?.x ?? 0,
        y: elkNode?.y ?? 0,
      },
    };
  });
}
