import type { FdmlDocument, Action } from '../api/types';

export interface InferredFlowEdge {
  fromAction: string;
  toAction: string;
  viaEntity: string;
}

export interface InferredFlow {
  id: string;
  name: string;
  rootActions: string[];
  edges: InferredFlowEdge[];
  actions: string[];
  entities: string[];
}

export function inferFlows(doc: FdmlDocument): InferredFlow[] {
  if (doc.actions.length === 0) return [];

  // Build producer/consumer maps
  // producerMap: entity → actions that output this entity
  // consumerMap: entity → actions that input this entity
  const producerMap = new Map<string, string[]>();
  const consumerMap = new Map<string, string[]>();
  const actionMap = new Map<string, Action>();

  for (const a of doc.actions) {
    actionMap.set(a.id, a);
    if (a.output?.entity) {
      const list = producerMap.get(a.output.entity) || [];
      list.push(a.id);
      producerMap.set(a.output.entity, list);
    }
    if (a.input?.entity) {
      const list = consumerMap.get(a.input.entity) || [];
      list.push(a.id);
      consumerMap.set(a.input.entity, list);
    }
  }

  // Detect "constructors" — actions where input.entity === output.entity
  const constructors = new Set<string>();
  for (const a of doc.actions) {
    if (a.input?.entity && a.output?.entity && a.input.entity === a.output.entity) {
      constructors.add(a.id);
    }
  }

  // Build edges: for each entity, connect producers → consumers (skip self-loops)
  const edges: InferredFlowEdge[] = [];
  const allEntities = new Set<string>();

  for (const [entity, producers] of producerMap) {
    const consumers = consumerMap.get(entity) || [];
    for (const p of producers) {
      for (const c of consumers) {
        if (p === c) continue; // no self-loops
        edges.push({ fromAction: p, toAction: c, viaEntity: entity });
        allEntities.add(entity);
      }
    }
  }

  if (edges.length === 0) return [];

  // Collect all actions that participate in edges
  const edgeActions = new Set<string>();
  for (const e of edges) {
    edgeActions.add(e.fromAction);
    edgeActions.add(e.toAction);
  }

  // Build adjacency list for BFS clustering (undirected)
  const adj = new Map<string, Set<string>>();
  for (const e of edges) {
    if (!adj.has(e.fromAction)) adj.set(e.fromAction, new Set());
    if (!adj.has(e.toAction)) adj.set(e.toAction, new Set());
    adj.get(e.fromAction)!.add(e.toAction);
    adj.get(e.toAction)!.add(e.fromAction);
  }

  // BFS to find connected components
  const visited = new Set<string>();
  const clusters: string[][] = [];

  for (const action of edgeActions) {
    if (visited.has(action)) continue;
    const cluster: string[] = [];
    const queue = [action];
    visited.add(action);
    while (queue.length > 0) {
      const current = queue.shift()!;
      cluster.push(current);
      for (const neighbor of adj.get(current) || []) {
        if (!visited.has(neighbor)) {
          visited.add(neighbor);
          queue.push(neighbor);
        }
      }
    }
    clusters.push(cluster);
  }

  // Build flows from clusters
  const entityNames = new Map<string, string>();
  for (const e of doc.entities) {
    entityNames.set(e.id, e.name || e.id);
  }

  return clusters.map((clusterActions, i) => {
    const clusterSet = new Set(clusterActions);
    const clusterEdges = edges.filter(
      (e) => clusterSet.has(e.fromAction) && clusterSet.has(e.toAction)
    );

    // Find entities involved
    const clusterEntities = new Set<string>();
    for (const e of clusterEdges) {
      clusterEntities.add(e.viaEntity);
    }

    // Find root actions: actions that are only producers (no incoming edges within cluster)
    const hasIncoming = new Set<string>();
    for (const e of clusterEdges) {
      hasIncoming.add(e.toAction);
    }
    const roots = clusterActions.filter((a) => !hasIncoming.has(a));

    // Name from primary entity (most connected)
    const entityCounts = new Map<string, number>();
    for (const e of clusterEdges) {
      entityCounts.set(e.viaEntity, (entityCounts.get(e.viaEntity) || 0) + 1);
    }
    let primaryEntity = '';
    let maxCount = 0;
    for (const [eid, count] of entityCounts) {
      if (count > maxCount) {
        maxCount = count;
        primaryEntity = eid;
      }
    }

    const primaryName = entityNames.get(primaryEntity) || primaryEntity;
    const name = `${toTitleCase(primaryName)} Lifecycle`;

    return {
      id: `inferred_flow_${i + 1}`,
      name,
      rootActions: roots,
      edges: clusterEdges,
      actions: clusterActions,
      entities: Array.from(clusterEntities),
    };
  });
}

function toTitleCase(s: string): string {
  return s
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}
