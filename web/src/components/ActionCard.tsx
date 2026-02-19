import { useState } from 'react';
import type { ActionNode } from '../layout/buildHierarchy';
import { NODE_COLORS } from '../theme/colors';

interface ActionCardProps {
  node: ActionNode;
  onEntityClick?: (entityId: string) => void;
}

export default function ActionCard({ node, onEntityClick }: ActionCardProps) {
  const [expanded, setExpanded] = useState(false);
  const { action, inputEntity, outputEntity } = node;
  const colors = NODE_COLORS.action;

  return (
    <div
      id={`action-${action.id}`}
      style={{
        borderLeft: `3px solid ${colors.border}`,
        background: '#fff',
        borderRadius: 6,
        marginBottom: 4,
        overflow: 'hidden',
      }}
    >
      <div
        onClick={() => setExpanded(!expanded)}
        style={{
          padding: '6px 10px',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          userSelect: 'none',
        }}
      >
        <span style={{ fontSize: 9, color: '#999', width: 10 }}>{expanded ? '\u25BC' : '\u25B6'}</span>
        <span style={{ fontWeight: 600, fontSize: 12, color: '#333', fontFamily: 'monospace' }}>
          {action.name || action.id}
        </span>
        <div style={{ flex: 1 }} />
        <EntityBadges
          inputEntity={inputEntity}
          outputEntity={outputEntity}
          onEntityClick={onEntityClick}
        />
      </div>
      {expanded && (
        <div style={{ padding: '0 10px 8px 24px', fontSize: 11 }}>
          {action.description && (
            <div style={{ color: '#666', marginBottom: 6 }}>{action.description}</div>
          )}

          {/* Input/Output details */}
          <div style={{ display: 'flex', gap: 16, marginBottom: 6 }}>
            {action.input && (
              <div>
                <span style={{ color: '#888', fontSize: 10, textTransform: 'uppercase', fontWeight: 500 }}>Input: </span>
                {action.input.entity ? (
                  <EntityLink
                    entityId={action.input.entity}
                    entityName={inputEntity?.name}
                    onClick={onEntityClick}
                  />
                ) : (
                  <span style={{ color: '#555' }}>{action.input.description || 'custom'}</span>
                )}
                {action.input.fields && action.input.fields.length > 0 && (
                  <span style={{ color: '#999', fontSize: 10, marginLeft: 4 }}>
                    [{action.input.fields.join(', ')}]
                  </span>
                )}
              </div>
            )}
            {action.output && (
              <div>
                <span style={{ color: '#888', fontSize: 10, textTransform: 'uppercase', fontWeight: 500 }}>Output: </span>
                {action.output.entity ? (
                  <EntityLink
                    entityId={action.output.entity}
                    entityName={outputEntity?.name}
                    onClick={onEntityClick}
                  />
                ) : (
                  <span style={{ color: '#555' }}>{action.output.description || 'custom'}</span>
                )}
                {action.output.fields && action.output.fields.length > 0 && (
                  <span style={{ color: '#999', fontSize: 10, marginLeft: 4 }}>
                    [{action.output.fields.join(', ')}]
                  </span>
                )}
              </div>
            )}
          </div>

          {/* Pre/postconditions */}
          {action.preconditions && action.preconditions.length > 0 && (
            <div style={{ marginBottom: 4 }}>
              <span style={{ color: '#888', fontSize: 10, textTransform: 'uppercase', fontWeight: 500 }}>Preconditions:</span>
              {action.preconditions.map((p, i) => (
                <div key={i} style={{ color: '#555', paddingLeft: 8, fontSize: 11 }}>- {p}</div>
              ))}
            </div>
          )}
          {action.postconditions && action.postconditions.length > 0 && (
            <div style={{ marginBottom: 4 }}>
              <span style={{ color: '#888', fontSize: 10, textTransform: 'uppercase', fontWeight: 500 }}>Postconditions:</span>
              {action.postconditions.map((p, i) => (
                <div key={i} style={{ color: '#555', paddingLeft: 8, fontSize: 11 }}>- {p}</div>
              ))}
            </div>
          )}
          {action.side_effects && action.side_effects.length > 0 && (
            <div>
              <span style={{ color: '#888', fontSize: 10, textTransform: 'uppercase', fontWeight: 500 }}>Side effects:</span>
              {action.side_effects.map((s, i) => (
                <div key={i} style={{ color: '#555', paddingLeft: 8, fontSize: 11 }}>- {s}</div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function EntityBadges({
  inputEntity,
  outputEntity,
  onEntityClick,
}: {
  inputEntity?: { id: string; name?: string };
  outputEntity?: { id: string; name?: string };
  onEntityClick?: (entityId: string) => void;
}) {
  const entities: Array<{ id: string; name?: string; label: string }> = [];
  if (inputEntity) entities.push({ ...inputEntity, label: 'in' });
  if (outputEntity && outputEntity.id !== inputEntity?.id) {
    entities.push({ ...outputEntity, label: 'out' });
  } else if (outputEntity && outputEntity.id === inputEntity?.id) {
    // Same entity for in/out — show one badge with "in/out"
    entities[0].label = 'in/out';
  }

  return (
    <div style={{ display: 'flex', gap: 4 }}>
      {entities.map((e) => (
        <span
          key={e.id + e.label}
          onClick={(ev) => {
            ev.stopPropagation();
            onEntityClick?.(e.id);
          }}
          style={{
            fontSize: 10,
            background: NODE_COLORS.entity.bg,
            color: NODE_COLORS.entity.badge,
            padding: '1px 6px',
            borderRadius: 3,
            cursor: onEntityClick ? 'pointer' : 'default',
            fontWeight: 500,
          }}
        >
          {e.label}: {e.name || e.id}
        </span>
      ))}
    </div>
  );
}

function EntityLink({
  entityId,
  entityName,
  onClick,
}: {
  entityId: string;
  entityName?: string;
  onClick?: (id: string) => void;
}) {
  return (
    <span
      onClick={(e) => {
        e.stopPropagation();
        onClick?.(entityId);
      }}
      style={{
        color: NODE_COLORS.entity.badge,
        cursor: onClick ? 'pointer' : 'default',
        fontWeight: 500,
        fontFamily: 'monospace',
      }}
    >
      {entityName || entityId}
    </span>
  );
}
