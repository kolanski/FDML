import type { FdmlDocument } from '../api/types';
import type { Hierarchy } from './buildHierarchy';

export type Severity = 'error' | 'warning' | 'info';

export interface HealthIssue {
  severity: Severity;
  message: string;
  elementType?: string;
  elementId?: string;
}

export interface CoverageMetrics {
  featuresWithActions: number;
  totalFeatures: number;
  entitiesUsed: number;
  totalEntities: number;
  actionsInFeatures: number;
  totalActions: number;
  constraintsLinked: number;
  totalConstraints: number;
}

export interface SpecHealth {
  score: number;
  issues: HealthIssue[];
  coverage: CoverageMetrics;
}

export function analyzeHealth(doc: FdmlDocument, hierarchy: Hierarchy): SpecHealth {
  const issues: HealthIssue[] = [];

  const entityIds = new Set(doc.entities.map((e) => e.id));
  const actionIds = new Set(doc.actions.map((a) => a.id));

  // Error: Dangling entity refs in actions
  for (const a of doc.actions) {
    if (a.input?.entity && !entityIds.has(a.input.entity)) {
      issues.push({
        severity: 'error',
        message: `Action "${a.name || a.id}" references unknown input entity "${a.input.entity}"`,
        elementType: 'action',
        elementId: a.id,
      });
    }
    if (a.output?.entity && !entityIds.has(a.output.entity)) {
      issues.push({
        severity: 'error',
        message: `Action "${a.name || a.id}" references unknown output entity "${a.output.entity}"`,
        elementType: 'action',
        elementId: a.id,
      });
    }
  }

  // Error: Dangling flow action refs
  for (const f of doc.flows) {
    for (const step of f.steps) {
      if (!actionIds.has(step.action)) {
        issues.push({
          severity: 'error',
          message: `Flow "${f.name}" step references unknown action "${step.action}"`,
          elementType: 'flow',
          elementId: f.id,
        });
      }
    }
  }

  // Warning: Orphan entities
  for (const eu of hierarchy.orphanEntities) {
    issues.push({
      severity: 'warning',
      message: `Entity "${eu.entity.name || eu.entity.id}" is not used by any action`,
      elementType: 'entity',
      elementId: eu.entity.id,
    });
  }

  // Warning: Orphan actions (not in any feature)
  for (const an of hierarchy.orphanActions) {
    issues.push({
      severity: 'warning',
      message: `Action "${an.action.name || an.action.id}" is not in any feature`,
      elementType: 'action',
      elementId: an.action.id,
    });
  }

  // Warning: Features with 0 actions
  for (const fn of hierarchy.features) {
    if (fn.actions.length === 0) {
      issues.push({
        severity: 'warning',
        message: `Feature "${fn.feature.title}" has no linked actions`,
        elementType: 'feature',
        elementId: fn.feature.id,
      });
    }
  }

  // Warning: Features with 0 scenarios
  for (const fn of hierarchy.features) {
    if (fn.feature.scenarios.length === 0) {
      issues.push({
        severity: 'warning',
        message: `Feature "${fn.feature.title}" has no scenarios`,
        elementType: 'feature',
        elementId: fn.feature.id,
      });
    }
  }

  // Info: Actions without input or output entity
  for (const a of doc.actions) {
    if (!a.input?.entity && !a.output?.entity) {
      issues.push({
        severity: 'info',
        message: `Action "${a.name || a.id}" has no entity references`,
        elementType: 'action',
        elementId: a.id,
      });
    }
  }

  // Info: Missing descriptions
  for (const e of doc.entities) {
    if (!e.description) {
      issues.push({
        severity: 'info',
        message: `Entity "${e.name || e.id}" is missing a description`,
        elementType: 'entity',
        elementId: e.id,
      });
    }
  }
  for (const a of doc.actions) {
    if (!a.description) {
      issues.push({
        severity: 'info',
        message: `Action "${a.name || a.id}" is missing a description`,
        elementType: 'action',
        elementId: a.id,
      });
    }
  }

  // Info: 0 explicit flows
  if (doc.flows.length === 0) {
    issues.push({
      severity: 'info',
      message: 'No explicit flows defined — consider adding flows to document entity chains',
    });
  }

  // Compute score: start at 100, -5/error, -2/warning, -1/info, floor 0
  const errorCount = issues.filter((i) => i.severity === 'error').length;
  const warningCount = issues.filter((i) => i.severity === 'warning').length;
  const infoCount = issues.filter((i) => i.severity === 'info').length;
  const score = Math.max(0, 100 - errorCount * 5 - warningCount * 2 - infoCount * 1);

  // Coverage metrics
  const featuresWithActions = hierarchy.features.filter((f) => f.actions.length > 0).length;
  const entitiesUsed = hierarchy.entities.length - hierarchy.orphanEntities.length;
  const actionsInFeatures = doc.actions.length - hierarchy.orphanActions.length;
  const constraintsLinked = doc.constraints.length - hierarchy.orphanConstraints.length;

  return {
    score,
    issues,
    coverage: {
      featuresWithActions,
      totalFeatures: doc.features.length,
      entitiesUsed,
      totalEntities: doc.entities.length,
      actionsInFeatures,
      totalActions: doc.actions.length,
      constraintsLinked,
      totalConstraints: doc.constraints.length,
    },
  };
}
