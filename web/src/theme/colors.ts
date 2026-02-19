export type NodeType = 'entity' | 'action' | 'feature' | 'constraint' | 'flow' | 'system';

export const NODE_COLORS: Record<NodeType, { bg: string; border: string; badge: string }> = {
  entity:     { bg: '#dbeafe', border: '#3b82f6', badge: '#2563eb' },
  action:     { bg: '#dcfce7', border: '#22c55e', badge: '#16a34a' },
  feature:    { bg: '#f3e8ff', border: '#a855f7', badge: '#9333ea' },
  constraint: { bg: '#fef3c7', border: '#f59e0b', badge: '#d97706' },
  flow:       { bg: '#ccfbf1', border: '#14b8a6', badge: '#0d9488' },
  system:     { bg: '#f3f4f6', border: '#6b7280', badge: '#4b5563' },
};

export const EDGE_COLORS = {
  entityRelation: '#3b82f6',
  traceability:   '#a855f7',
  actionEntity:   '#22c55e',
  flowStep:       '#14b8a6',
  constraintTarget: '#f59e0b',
};
