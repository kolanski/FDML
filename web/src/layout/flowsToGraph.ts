import type { Node, Edge } from '@xyflow/react';
import type { FdmlDocument, Flow, Action } from '../api/types';
import type { InferredFlow } from './inferFlows';
import { EDGE_COLORS, NODE_COLORS } from '../theme/colors';

export interface FlowGraphData {
  nodeType: 'action';
  label: string;
  fdmlId: string;
  isRoot: boolean;
  flowId: string;
}

export function flowsToGraph(
  doc: FdmlDocument,
  inferredFlows: InferredFlow[],
  selectedFlowId: string | null,
): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = [];
  const edges: Edge[] = [];
  const actionMap = new Map<string, Action>();
  for (const a of doc.actions) actionMap.set(a.id, a);

  // Combine explicit flows and inferred flows
  const allFlows: Array<{
    id: string;
    name: string;
    type: 'explicit' | 'inferred';
    actionIds: string[];
    rootActions: string[];
    flowEdges: Array<{ from: string; to: string; via?: string }>;
  }> = [];

  // Explicit flows
  for (const f of doc.flows) {
    const actionIds = f.steps.map((s) => s.action);
    const flowEdges: Array<{ from: string; to: string; via?: string }> = [];
    for (let i = 0; i < f.steps.length - 1; i++) {
      flowEdges.push({ from: f.steps[i].action, to: f.steps[i + 1].action });
    }
    allFlows.push({
      id: f.id,
      name: f.name,
      type: 'explicit',
      actionIds,
      rootActions: actionIds.length > 0 ? [actionIds[0]] : [],
      flowEdges,
    });
  }

  // Inferred flows
  for (const inf of inferredFlows) {
    allFlows.push({
      id: inf.id,
      name: inf.name,
      type: 'inferred',
      actionIds: inf.actions,
      rootActions: inf.rootActions,
      flowEdges: inf.edges.map((e) => ({ from: e.fromAction, to: e.toAction, via: e.viaEntity })),
    });
  }

  // Filter to selected flow if any
  const flows = selectedFlowId
    ? allFlows.filter((f) => f.id === selectedFlowId)
    : allFlows;

  const addedNodes = new Set<string>();

  for (const flow of flows) {
    for (const aid of flow.actionIds) {
      const nodeId = `flow-action::${aid}`;
      if (addedNodes.has(nodeId)) continue;
      addedNodes.add(nodeId);

      const action = actionMap.get(aid);
      const isRoot = flow.rootActions.includes(aid);
      nodes.push({
        id: nodeId,
        type: 'specNode',
        position: { x: 0, y: 0 },
        data: {
          nodeType: 'action',
          label: action?.name || aid,
          fdmlId: aid,
          expanded: false,
          spec: action || { id: aid },
          counts: {},
          isRoot,
        },
        style: isRoot
          ? { border: `3px solid ${NODE_COLORS.action.border}`, borderRadius: 8 }
          : undefined,
      });
    }

    for (let i = 0; i < flow.flowEdges.length; i++) {
      const e = flow.flowEdges[i];
      const sourceId = `flow-action::${e.from}`;
      const targetId = `flow-action::${e.to}`;
      if (!addedNodes.has(sourceId) || !addedNodes.has(targetId)) continue;

      edges.push({
        id: `${flow.id}::${e.from}->${e.to}::${i}`,
        source: sourceId,
        target: targetId,
        type: 'default',
        label: e.via || '',
        style: { stroke: EDGE_COLORS.flowStep, strokeWidth: 2 },
        markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.flowStep },
      });
    }
  }

  return { nodes, edges };
}

export function getFlowList(
  doc: FdmlDocument,
  inferredFlows: InferredFlow[],
): Array<{ id: string; name: string; type: 'explicit' | 'inferred' }> {
  const list: Array<{ id: string; name: string; type: 'explicit' | 'inferred' }> = [];
  for (const f of doc.flows) {
    list.push({ id: f.id, name: f.name, type: 'explicit' });
  }
  for (const f of inferredFlows) {
    list.push({ id: f.id, name: f.name, type: 'inferred' });
  }
  return list;
}
