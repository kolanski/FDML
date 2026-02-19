import type { Node, Edge } from '@xyflow/react';
import type { FdmlDocument } from '../api/types';
import { EDGE_COLORS } from '../theme/colors';

export interface SpecNodeData {
  nodeType: 'entity' | 'action' | 'feature' | 'constraint' | 'flow' | 'system';
  label: string;
  fdmlId: string;
  expanded: boolean;
  spec: unknown; // the raw FDML object for this node
  counts: Record<string, number>;
}

export function specToGraph(doc: FdmlDocument): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = [];
  const edges: Edge[] = [];
  const idSet = new Set<string>();

  // Helper to create unique node id
  const nid = (type: string, id: string) => `${type}::${id}`;

  // System node
  if (doc.system) {
    const sid = nid('system', doc.system.id);
    idSet.add(sid);
    nodes.push({
      id: sid,
      type: 'specNode',
      position: { x: 0, y: 0 },
      data: {
        nodeType: 'system',
        label: doc.system.name,
        fdmlId: doc.system.id,
        expanded: false,
        spec: doc.system,
        counts: {
          components: doc.system.components.length,
          relationships: doc.system.relationships.length,
        },
      } satisfies SpecNodeData,
    });
  }

  // Features
  for (const f of doc.features) {
    const id = nid('feature', f.id);
    idSet.add(id);
    nodes.push({
      id,
      type: 'specNode',
      position: { x: 0, y: 0 },
      data: {
        nodeType: 'feature',
        label: f.title,
        fdmlId: f.id,
        expanded: false,
        spec: f,
        counts: { scenarios: f.scenarios.length },
      } satisfies SpecNodeData,
    });
  }

  // Actions
  for (const a of doc.actions) {
    const id = nid('action', a.id);
    idSet.add(id);
    const counts: Record<string, number> = {};
    if (a.preconditions) counts.preconditions = a.preconditions.length;
    if (a.postconditions) counts.postconditions = a.postconditions.length;
    nodes.push({
      id,
      type: 'specNode',
      position: { x: 0, y: 0 },
      data: {
        nodeType: 'action',
        label: a.name || a.id,
        fdmlId: a.id,
        expanded: false,
        spec: a,
        counts,
      } satisfies SpecNodeData,
    });

    // Action → Entity edges (input/output)
    if (a.input?.entity) {
      const targetId = nid('entity', a.input.entity);
      edges.push({
        id: `${id}->input->${targetId}`,
        source: id,
        target: targetId,
        type: 'default',
        label: 'input',
        style: { stroke: EDGE_COLORS.actionEntity, strokeWidth: 1.5 },
        markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.actionEntity },
      });
    }
    if (a.output?.entity) {
      const targetId = nid('entity', a.output.entity);
      edges.push({
        id: `${id}->output->${targetId}`,
        source: id,
        target: targetId,
        type: 'default',
        label: 'output',
        style: { stroke: EDGE_COLORS.actionEntity, strokeWidth: 1.5 },
        markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.actionEntity },
      });
    }
  }

  // Entities
  for (const e of doc.entities) {
    const id = nid('entity', e.id);
    idSet.add(id);
    const counts: Record<string, number> = { fields: e.fields.length };
    if (e.relationships) counts.relationships = e.relationships.length;
    nodes.push({
      id,
      type: 'specNode',
      position: { x: 0, y: 0 },
      data: {
        nodeType: 'entity',
        label: e.name || e.id,
        fdmlId: e.id,
        expanded: false,
        spec: e,
        counts,
      } satisfies SpecNodeData,
    });

    // Entity → Entity relationship edges
    if (e.relationships) {
      for (const rel of e.relationships) {
        const targetId = nid('entity', rel.entity);
        edges.push({
          id: `${id}->${rel.type}->${targetId}`,
          source: id,
          target: targetId,
          type: 'default',
          label: rel.type,
          style: { stroke: EDGE_COLORS.entityRelation, strokeWidth: 2 },
          markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.entityRelation },
        });
      }
    }
  }

  // Constraints
  for (const c of doc.constraints) {
    const id = nid('constraint', c.id);
    idSet.add(id);
    nodes.push({
      id,
      type: 'specNode',
      position: { x: 0, y: 0 },
      data: {
        nodeType: 'constraint',
        label: c.name,
        fdmlId: c.id,
        expanded: false,
        spec: c,
        counts: {},
      } satisfies SpecNodeData,
    });

    // Constraint → Entity edges
    if (c.entities) {
      for (const eid of c.entities) {
        const targetId = nid('entity', eid);
        edges.push({
          id: `${id}->constrains->${targetId}`,
          source: id,
          target: targetId,
          type: 'default',
          label: 'constrains',
          style: { stroke: EDGE_COLORS.constraintTarget, strokeWidth: 1, strokeDasharray: '4 2' },
          markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.constraintTarget },
        });
      }
    }
    // Constraint → Action edges
    if (c.actions) {
      for (const aid of c.actions) {
        const targetId = nid('action', aid);
        edges.push({
          id: `${id}->constrains->${targetId}`,
          source: id,
          target: targetId,
          type: 'default',
          label: 'constrains',
          style: { stroke: EDGE_COLORS.constraintTarget, strokeWidth: 1, strokeDasharray: '4 2' },
          markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.constraintTarget },
        });
      }
    }
  }

  // Flows
  for (const f of doc.flows) {
    const id = nid('flow', f.id);
    idSet.add(id);
    nodes.push({
      id,
      type: 'specNode',
      position: { x: 0, y: 0 },
      data: {
        nodeType: 'flow',
        label: f.name,
        fdmlId: f.id,
        expanded: false,
        spec: f,
        counts: { steps: f.steps.length },
      } satisfies SpecNodeData,
    });

    // Flow step → action edges
    for (let i = 0; i < f.steps.length; i++) {
      const step = f.steps[i];
      const actionTarget = nid('action', step.action);
      if (idSet.has(actionTarget) || doc.actions.some(a => a.id === step.action)) {
        edges.push({
          id: `${id}->step${i}->${actionTarget}`,
          source: id,
          target: actionTarget,
          type: 'default',
          label: `step ${i + 1}`,
          style: { stroke: EDGE_COLORS.flowStep, strokeWidth: 2.5 },
          markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.flowStep },
        });
      }
    }
  }

  // Traceability edges
  for (const t of doc.traceability) {
    // Find source and target nodes by fdml id
    const sourceNode = nodes.find(n => (n.data as unknown as SpecNodeData).fdmlId === t.from);
    const targetNode = nodes.find(n => (n.data as unknown as SpecNodeData).fdmlId === t.to);
    if (sourceNode && targetNode) {
      edges.push({
        id: `trace::${t.from}->${t.relation}->${t.to}`,
        source: sourceNode.id,
        target: targetNode.id,
        type: 'default',
        label: t.relation,
        style: { stroke: EDGE_COLORS.traceability, strokeWidth: 1.5, strokeDasharray: '6 3' },
        markerEnd: { type: 'arrowclosed' as const, color: EDGE_COLORS.traceability },
      });
    }
  }

  return { nodes, edges };
}
