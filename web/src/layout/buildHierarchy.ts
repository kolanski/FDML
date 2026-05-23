import type { FdmlDocument, Feature, Action, Entity, Constraint, Flow } from '../api/types';
import { inferFlows, type InferredFlow } from './inferFlows';

export interface ActionNode {
  action: Action;
  inputEntity?: Entity;
  outputEntity?: Entity;
  relatedEntityIds: string[];
}

export interface FeatureNode {
  feature: Feature;
  actions: ActionNode[];
  constraints: Constraint[];
}

export interface EntityUsage {
  entity: Entity;
  usedByActions: Action[];
  usedByConstraints: Constraint[];
}

export interface FlowNode {
  flow: Flow;
  actions: Action[];
}

export interface Hierarchy {
  features: FeatureNode[];
  orphanActions: ActionNode[];
  entities: EntityUsage[];
  orphanEntities: EntityUsage[];
  constraints: Constraint[];
  orphanConstraints: Constraint[];
  flows: FlowNode[];
  inferredFlows: InferredFlow[];
}

export function buildHierarchy(doc: FdmlDocument): Hierarchy {
  const entityMap = new Map<string, Entity>();
  for (const e of doc.entities) entityMap.set(e.id, e);

  const actionMap = new Map<string, Action>();
  for (const a of doc.actions) actionMap.set(a.id, a);

  const constraintMap = new Map<string, Constraint>();
  for (const c of doc.constraints) constraintMap.set(c.id, c);

  // Build action→entity relationships
  const actionEntityIds = new Map<string, Set<string>>();
  for (const a of doc.actions) {
    const ids = new Set<string>();
    if (a.input?.entity) ids.add(a.input.entity);
    if (a.output?.entity) ids.add(a.output.entity);
    actionEntityIds.set(a.id, ids);
  }

  // Build feature→action links from multiple sources
  const featureActionIds = new Map<string, Set<string>>();
  for (const f of doc.features) {
    featureActionIds.set(f.id, new Set<string>());
  }

  // Source 1: Traceability links (feature→action with relation "implements" or similar)
  for (const t of doc.traceability) {
    if (featureActionIds.has(t.from) && actionMap.has(t.to)) {
      featureActionIds.get(t.from)!.add(t.to);
    }
  }

  // Source 2: Action IDs mentioned in feature scenario text
  const actionIds = Array.from(actionMap.keys());
  for (const f of doc.features) {
    const featureText = getFeatureText(f);
    for (const aid of actionIds) {
      if (fuzzyMatchActionInText(aid, actionMap.get(aid)!, featureText)) {
        featureActionIds.get(f.id)!.add(aid);
      }
    }
  }

  // Source 3: Constraints that reference actions — link those actions to the feature
  // if the constraint's entities overlap with entities used by a feature's actions
  const featureConstraintIds = new Map<string, Set<string>>();
  for (const f of doc.features) {
    featureConstraintIds.set(f.id, new Set<string>());
  }

  // Build constraint→feature associations via entity overlap
  for (const c of doc.constraints) {
    if (c.actions) {
      for (const aid of c.actions) {
        // Find which feature this action belongs to
        for (const [fid, aids] of featureActionIds) {
          if (aids.has(aid)) {
            featureConstraintIds.get(fid)!.add(c.id);
          }
        }
      }
    }
    if (c.entities) {
      // Link constraint to features whose actions use these entities
      for (const eid of c.entities) {
        for (const [fid, aids] of featureActionIds) {
          for (const aid of aids) {
            const entityIds = actionEntityIds.get(aid);
            if (entityIds?.has(eid)) {
              featureConstraintIds.get(fid)!.add(c.id);
            }
          }
        }
      }
    }
  }

  // Build ActionNode helper
  function buildActionNode(action: Action): ActionNode {
    const relatedEntityIds: string[] = [];
    if (action.input?.entity) relatedEntityIds.push(action.input.entity);
    if (action.output?.entity && action.output.entity !== action.input?.entity) {
      relatedEntityIds.push(action.output.entity);
    }
    return {
      action,
      inputEntity: action.input?.entity ? entityMap.get(action.input.entity) : undefined,
      outputEntity: action.output?.entity ? entityMap.get(action.output.entity) : undefined,
      relatedEntityIds,
    };
  }

  // Build feature nodes
  const claimedActionIds = new Set<string>();
  const claimedConstraintIds = new Set<string>();

  const features: FeatureNode[] = doc.features.map((f) => {
    const aids = featureActionIds.get(f.id)!;
    const actions: ActionNode[] = [];
    for (const aid of aids) {
      const action = actionMap.get(aid);
      if (action) {
        actions.push(buildActionNode(action));
        claimedActionIds.add(aid);
      }
    }

    const cids = featureConstraintIds.get(f.id)!;
    const constraints: Constraint[] = [];
    for (const cid of cids) {
      const constraint = constraintMap.get(cid);
      if (constraint) {
        constraints.push(constraint);
        claimedConstraintIds.add(cid);
      }
    }

    return { feature: f, actions, constraints };
  });

  // Orphan actions
  const orphanActions: ActionNode[] = [];
  for (const a of doc.actions) {
    if (!claimedActionIds.has(a.id)) {
      orphanActions.push(buildActionNode(a));
    }
  }

  // Build entity usage
  const entityUsageMap = new Map<string, EntityUsage>();
  for (const e of doc.entities) {
    entityUsageMap.set(e.id, {
      entity: e,
      usedByActions: [],
      usedByConstraints: [],
    });
  }

  for (const a of doc.actions) {
    if (a.input?.entity) {
      entityUsageMap.get(a.input.entity)?.usedByActions.push(a);
    }
    if (a.output?.entity && a.output.entity !== a.input?.entity) {
      entityUsageMap.get(a.output.entity)?.usedByActions.push(a);
    }
  }

  for (const c of doc.constraints) {
    if (c.entities) {
      for (const eid of c.entities) {
        entityUsageMap.get(eid)?.usedByConstraints.push(c);
      }
    }
  }

  // Split entities into used vs orphan
  const entities: EntityUsage[] = [];
  const orphanEntities: EntityUsage[] = [];
  for (const eu of entityUsageMap.values()) {
    if (eu.usedByActions.length > 0 || eu.usedByConstraints.length > 0) {
      entities.push(eu);
    } else {
      orphanEntities.push(eu);
    }
  }

  // Orphan constraints
  const orphanConstraints = doc.constraints.filter((c) => !claimedConstraintIds.has(c.id));

  // Flows
  const flows: FlowNode[] = doc.flows.map((f) => ({
    flow: f,
    actions: f.steps.map((s) => actionMap.get(s.action)).filter(Boolean) as Action[],
  }));

  // Inferred flows
  const inferredFlows = inferFlows(doc);

  return {
    features,
    orphanActions,
    entities: [...entities, ...orphanEntities],
    orphanEntities,
    constraints: doc.constraints,
    orphanConstraints,
    flows,
    inferredFlows,
  };
}

function getFeatureText(f: Feature): string {
  const parts = [f.title, f.description || ''];
  for (const s of f.scenarios) {
    parts.push(s.title, s.description || '');
    parts.push(...s.given, ...s.when, ...s.then);
  }
  if (f.acceptance_criteria) parts.push(...f.acceptance_criteria);
  return parts.join(' ').toLowerCase();
}

function fuzzyMatchActionInText(actionId: string, action: Action, text: string): boolean {
  // Match action ID directly (e.g. "create_workflow" in text)
  if (text.includes(actionId.toLowerCase())) return true;

  // Match action name words (e.g. "Create Workflow" → look for "create" AND "workflow")
  if (action.name) {
    const words = action.name.toLowerCase().split(/\s+/).filter((w) => w.length > 3);
    if (words.length >= 2 && words.every((w) => text.includes(w))) return true;
  }

  return false;
}
