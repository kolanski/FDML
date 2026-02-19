import { useState } from 'react';
import type { Action, Constraint } from '../api/types';
import type { EntityUsage } from '../layout/buildHierarchy';
import { NODE_COLORS } from '../theme/colors';

interface EntityCardProps {
  usage: EntityUsage;
  onActionClick?: (actionId: string) => void;
}

export default function EntityCard({ usage, onActionClick }: EntityCardProps) {
  const [expanded, setExpanded] = useState(false);
  const { entity, usedByActions, usedByConstraints } = usage;
  const colors = NODE_COLORS.entity;
  const fieldCount = entity.fields.length;

  return (
    <div
      id={`entity-${entity.id}`}
      style={{
        borderLeft: `3px solid ${colors.border}`,
        background: '#fff',
        borderRadius: 6,
        marginBottom: 8,
        boxShadow: '0 1px 3px rgba(0,0,0,0.06)',
        overflow: 'hidden',
      }}
    >
      <div
        onClick={() => setExpanded(!expanded)}
        style={{
          padding: '10px 14px',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          userSelect: 'none',
        }}
      >
        <span style={{ fontSize: 10, color: '#999', width: 12 }}>{expanded ? '\u25BC' : '\u25B6'}</span>
        <span style={{ fontWeight: 600, fontSize: 13, color: '#333' }}>
          {entity.name || entity.id}
        </span>
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
          {fieldCount} field{fieldCount !== 1 ? 's' : ''}
        </span>
        {usedByActions.length > 0 && (
          <span style={{ fontSize: 10, color: '#888' }}>
            {usedByActions.length} action{usedByActions.length !== 1 ? 's' : ''}
          </span>
        )}
      </div>
      {expanded && (
        <div style={{ padding: '0 14px 12px 32px', fontSize: 12 }}>
          {entity.description && (
            <div style={{ color: '#666', marginBottom: 8 }}>{entity.description}</div>
          )}

          {/* Fields table */}
          <div style={{ marginBottom: 8 }}>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11 }}>
              <thead>
                <tr style={{ borderBottom: '1px solid #e5e7eb' }}>
                  <th style={{ textAlign: 'left', padding: '4px 8px', color: '#888', fontWeight: 500 }}>Field</th>
                  <th style={{ textAlign: 'left', padding: '4px 8px', color: '#888', fontWeight: 500 }}>Type</th>
                  <th style={{ textAlign: 'left', padding: '4px 8px', color: '#888', fontWeight: 500 }}>Req</th>
                  <th style={{ textAlign: 'left', padding: '4px 8px', color: '#888', fontWeight: 500 }}>Description</th>
                </tr>
              </thead>
              <tbody>
                {entity.fields.map((f) => (
                  <tr key={f.name} style={{ borderBottom: '1px solid #f3f4f6' }}>
                    <td style={{ padding: '3px 8px', fontFamily: 'monospace', fontWeight: 500 }}>{f.name}</td>
                    <td style={{ padding: '3px 8px', fontFamily: 'monospace', color: '#7c3aed' }}>{f.type}</td>
                    <td style={{ padding: '3px 8px', color: f.required ? '#059669' : '#999' }}>
                      {f.required ? 'yes' : 'no'}
                    </td>
                    <td style={{ padding: '3px 8px', color: '#666', maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {f.description || ''}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Relationships */}
          {entity.relationships && entity.relationships.length > 0 && (
            <div style={{ marginBottom: 8 }}>
              <div style={{ fontWeight: 500, color: '#888', marginBottom: 4, fontSize: 10, textTransform: 'uppercase', letterSpacing: 0.5 }}>
                Relationships
              </div>
              {entity.relationships.map((r, i) => (
                <div key={i} style={{ color: '#555', fontSize: 11 }}>
                  {r.type} <span style={{ fontFamily: 'monospace', color: colors.badge }}>{r.entity}</span>
                  {r.description && <span style={{ color: '#999' }}> — {r.description}</span>}
                </div>
              ))}
            </div>
          )}

          {/* Used by actions */}
          {usedByActions.length > 0 && (
            <div style={{ marginBottom: 4 }}>
              <span style={{ fontSize: 10, color: '#888', textTransform: 'uppercase', letterSpacing: 0.5, fontWeight: 500 }}>
                Used by:{' '}
              </span>
              {usedByActions.map((a: Action, i: number) => (
                <span key={a.id}>
                  {i > 0 && ', '}
                  <span
                    onClick={(e) => {
                      e.stopPropagation();
                      onActionClick?.(a.id);
                    }}
                    style={{
                      color: NODE_COLORS.action.badge,
                      cursor: onActionClick ? 'pointer' : 'default',
                      fontWeight: 500,
                      fontSize: 11,
                    }}
                  >
                    {a.name || a.id}
                  </span>
                </span>
              ))}
            </div>
          )}

          {/* Used by constraints */}
          {usedByConstraints.length > 0 && (
            <div>
              <span style={{ fontSize: 10, color: '#888', textTransform: 'uppercase', letterSpacing: 0.5, fontWeight: 500 }}>
                Constrained by:{' '}
              </span>
              {usedByConstraints.map((c: Constraint, i: number) => (
                <span key={c.id}>
                  {i > 0 && ', '}
                  <span style={{ color: NODE_COLORS.constraint.badge, fontWeight: 500, fontSize: 11 }}>
                    {c.name}
                  </span>
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
