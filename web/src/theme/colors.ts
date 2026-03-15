export type NodeType = 'entity' | 'action' | 'feature' | 'constraint' | 'flow' | 'system'
  | 'contour' | 'integration' | 'cross_flow' | 'shared_entity';

export const NODE_COLORS: Record<NodeType, { bg: string; border: string; badge: string }> = {
  entity:        { bg: '#dbeafe', border: '#3b82f6', badge: '#2563eb' },
  action:        { bg: '#dcfce7', border: '#22c55e', badge: '#16a34a' },
  feature:       { bg: '#f3e8ff', border: '#a855f7', badge: '#9333ea' },
  constraint:    { bg: '#fef3c7', border: '#f59e0b', badge: '#d97706' },
  flow:          { bg: '#ccfbf1', border: '#14b8a6', badge: '#0d9488' },
  system:        { bg: '#f3f4f6', border: '#6b7280', badge: '#4b5563' },
  contour:       { bg: '#e0e7ff', border: '#6366f1', badge: '#4f46e5' },
  integration:   { bg: '#fce7f3', border: '#ec4899', badge: '#db2777' },
  cross_flow:    { bg: '#fef9c3', border: '#eab308', badge: '#ca8a04' },
  shared_entity: { bg: '#e0f2fe', border: '#0ea5e9', badge: '#0284c7' },
};

export const EDGE_COLORS = {
  entityRelation: '#3b82f6',
  traceability:   '#a855f7',
  actionEntity:   '#22c55e',
  flowStep:       '#14b8a6',
  constraintTarget: '#f59e0b',
  integration:    '#ec4899',
  crossFlow:      '#eab308',
};

// Icons for system types
export const SYSTEM_TYPE_ICONS: Record<string, string> = {
  frontend: '\uD83D\uDCBB',
  gateway:  '\uD83D\uDD00',
  service:  '\u2699\uFE0F',
  worker:   '\u26A1',
  database: '\uD83D\uDDC4\uFE0F',
  queue:    '\uD83D\uDCE8',
  storage:  '\uD83D\uDCC1',
  external: '\uD83C\uDF10',
};
