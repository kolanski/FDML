import { useState } from 'react';
import type { Constraint } from '../api/types';
import { NODE_COLORS } from '../theme/colors';

interface ConstraintCardProps {
  constraint: Constraint;
  compact?: boolean;
}

export default function ConstraintCard({ constraint, compact }: ConstraintCardProps) {
  const [expanded, setExpanded] = useState(false);
  const colors = NODE_COLORS.constraint;

  if (compact) {
    return (
      <div style={{ fontSize: 12, color: '#666', padding: '2px 0' }}>
        <div
          onClick={() => setExpanded(!expanded)}
          style={{ cursor: 'pointer', userSelect: 'none', display: 'flex', alignItems: 'baseline', gap: 4 }}
        >
          <span style={{ fontSize: 9, color: '#999', width: 10, flexShrink: 0 }}>{expanded ? '\u25BC' : '\u25B6'}</span>
          <span style={{ color: colors.badge, fontWeight: 500 }}>{constraint.name}</span>
          {constraint.entities && constraint.entities.length > 0 && (
            <span style={{ color: '#999', marginLeft: 4 }}>
              ({constraint.entities.join(', ')})
            </span>
          )}
        </div>
        {expanded && (
          <div style={{ padding: '4px 0 4px 14px', fontSize: 11 }}>
            {constraint.description && (
              <div style={{ color: '#555', marginBottom: 4 }}>{constraint.description}</div>
            )}
            {constraint.rule && (
              <div style={{ color: '#555', fontFamily: 'monospace', fontSize: 10, background: '#f9fafb', padding: '4px 6px', borderRadius: 3 }}>
                {constraint.rule}
              </div>
            )}
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      style={{
        borderLeft: `3px solid ${colors.border}`,
        background: '#fff',
        borderRadius: 6,
        marginBottom: 6,
        overflow: 'hidden',
      }}
    >
      <div
        onClick={() => setExpanded(!expanded)}
        style={{
          padding: '8px 12px',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          userSelect: 'none',
        }}
      >
        <span style={{ fontSize: 10, color: '#999', width: 12 }}>{expanded ? '\u25BC' : '\u25B6'}</span>
        <span style={{ fontWeight: 600, fontSize: 13, color: '#333' }}>{constraint.name}</span>
        <span
          style={{
            fontSize: 10,
            background: colors.bg,
            color: colors.badge,
            padding: '1px 6px',
            borderRadius: 3,
            fontWeight: 500,
          }}
        >
          {constraint.type}
        </span>
      </div>
      {expanded && (
        <div style={{ padding: '0 12px 10px 32px', fontSize: 12 }}>
          {constraint.description && (
            <div style={{ color: '#666', marginBottom: 6 }}>{constraint.description}</div>
          )}
          <div style={{ color: '#555', fontFamily: 'monospace', fontSize: 11, background: '#f9fafb', padding: '6px 8px', borderRadius: 4 }}>
            {constraint.rule}
          </div>
          {constraint.entities && constraint.entities.length > 0 && (
            <div style={{ marginTop: 6, color: '#888', fontSize: 11 }}>
              Entities: {constraint.entities.join(', ')}
            </div>
          )}
          {constraint.actions && constraint.actions.length > 0 && (
            <div style={{ marginTop: 2, color: '#888', fontSize: 11 }}>
              Actions: {constraint.actions.join(', ')}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
