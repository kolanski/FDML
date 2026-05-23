import { useState, useMemo } from 'react';
import type { FeatureNode } from '../layout/buildHierarchy';
import { NODE_COLORS } from '../theme/colors';
import ActionCard from './ActionCard';
import ConstraintCard from './ConstraintCard';

interface FeatureCardProps {
  node: FeatureNode;
  onEntityClick?: (entityId: string) => void;
  onActionClick?: (actionId: string) => void;
  defaultExpanded?: boolean;
}

export default function FeatureCard({ node, onEntityClick, onActionClick, defaultExpanded }: FeatureCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded || false);
  const { feature, actions, constraints } = node;
  const colors = NODE_COLORS.feature;

  // Build mini flow chains for this feature's actions
  const miniFlowEdges = useMemo(() => {
    if (actions.length < 2) return [];
    const edges: Array<{ from: string; to: string; via: string }> = [];
    for (const a of actions) {
      for (const b of actions) {
        if (a.action.id === b.action.id) continue;
        if (a.action.output?.entity && b.action.input?.entity && a.action.output.entity === b.action.input.entity) {
          edges.push({ from: a.action.id, to: b.action.id, via: a.action.output.entity });
        }
      }
    }
    return edges;
  }, [actions]);

  const actionName = (id: string) => {
    const a = actions.find((an) => an.action.id === id);
    return a?.action.name || id;
  };

  return (
    <div
      id={`feature-${feature.id}`}
      style={{
        borderLeft: `3px solid ${colors.border}`,
        background: '#fff',
        borderRadius: 8,
        marginBottom: 10,
        boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
        overflow: 'hidden',
      }}
    >
      {/* Header */}
      <div
        onClick={() => setExpanded(!expanded)}
        style={{
          padding: '12px 16px',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          userSelect: 'none',
        }}
      >
        <span style={{ fontSize: 11, color: '#999', width: 14 }}>{expanded ? '\u25BC' : '\u25B6'}</span>
        <div style={{ flex: 1 }}>
          <div style={{ fontWeight: 600, fontSize: 14, color: '#1f2937' }}>
            {feature.title}
          </div>
          {!expanded && feature.description && (
            <div style={{ fontSize: 12, color: '#888', marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 600 }}>
              {feature.description}
            </div>
          )}
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          {actions.length > 0 && (
            <Badge color={NODE_COLORS.action.badge} bg={NODE_COLORS.action.bg}>
              {actions.length} action{actions.length !== 1 ? 's' : ''}
            </Badge>
          )}
          {feature.scenarios.length > 0 && (
            <Badge color={colors.badge} bg={colors.bg}>
              {feature.scenarios.length} scenario{feature.scenarios.length !== 1 ? 's' : ''}
            </Badge>
          )}
        </div>
      </div>

      {/* Expanded content */}
      {expanded && (
        <div style={{ padding: '0 16px 14px 16px' }}>
          {feature.description && (
            <div style={{ fontSize: 12, color: '#666', marginBottom: 12, paddingLeft: 24 }}>
              {feature.description}
            </div>
          )}

          {/* Mini flow chain */}
          {miniFlowEdges.length > 0 && (
            <div style={{ paddingLeft: 24, marginBottom: 12 }}>
              <SectionHeader>Data Flow</SectionHeader>
              <div style={{ background: '#f9fafb', borderRadius: 4, padding: '6px 10px' }}>
                {miniFlowEdges.map((edge, i) => (
                  <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 11, padding: '1px 0' }}>
                    <span
                      onClick={(e) => { e.stopPropagation(); onActionClick?.(edge.from); }}
                      style={{ fontFamily: 'monospace', color: NODE_COLORS.action.badge, cursor: 'pointer', fontWeight: 500 }}
                    >
                      {actionName(edge.from)}
                    </span>
                    <span style={{ color: '#ccc' }}>--[</span>
                    <span
                      onClick={(e) => { e.stopPropagation(); onEntityClick?.(edge.via); }}
                      style={{ color: NODE_COLORS.entity.badge, cursor: 'pointer', fontFamily: 'monospace', fontSize: 10 }}
                    >
                      {edge.via}
                    </span>
                    <span style={{ color: '#ccc' }}>]--&gt;</span>
                    <span
                      onClick={(e) => { e.stopPropagation(); onActionClick?.(edge.to); }}
                      style={{ fontFamily: 'monospace', color: NODE_COLORS.action.badge, cursor: 'pointer', fontWeight: 500 }}
                    >
                      {actionName(edge.to)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Actions */}
          {actions.length > 0 && (
            <div style={{ marginBottom: 12, paddingLeft: 24 }}>
              <SectionHeader>Actions ({actions.length})</SectionHeader>
              {actions.map((an) => (
                <ActionCard key={an.action.id} node={an} onEntityClick={onEntityClick} />
              ))}
            </div>
          )}

          {/* Scenarios */}
          {feature.scenarios.length > 0 && (
            <div style={{ marginBottom: 12, paddingLeft: 24 }}>
              <SectionHeader>Scenarios ({feature.scenarios.length})</SectionHeader>
              {feature.scenarios.map((s) => (
                <ScenarioItem key={s.id} scenario={s} />
              ))}
            </div>
          )}

          {/* Constraints */}
          {constraints.length > 0 && (
            <div style={{ paddingLeft: 24 }}>
              <SectionHeader>Constraints ({constraints.length})</SectionHeader>
              {constraints.map((c) => (
                <ConstraintCard key={c.id} constraint={c} compact />
              ))}
            </div>
          )}

          {/* Dependencies */}
          {feature.dependencies && feature.dependencies.length > 0 && (
            <div style={{ paddingLeft: 24, marginTop: 8 }}>
              <SectionHeader>Dependencies</SectionHeader>
              <div style={{ fontSize: 11, color: '#666' }}>
                {feature.dependencies.join(', ')}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        fontSize: 10,
        fontWeight: 600,
        textTransform: 'uppercase',
        letterSpacing: 0.8,
        color: '#999',
        marginBottom: 6,
      }}
    >
      {children}
    </div>
  );
}

function Badge({ children, color, bg }: { children: React.ReactNode; color: string; bg: string }) {
  return (
    <span
      style={{
        fontSize: 10,
        background: bg,
        color,
        padding: '2px 8px',
        borderRadius: 4,
        fontWeight: 500,
      }}
    >
      {children}
    </span>
  );
}

function ScenarioItem({ scenario }: { scenario: { id: string; title: string; given: string[]; when: string[]; then: string[] } }) {
  const [open, setOpen] = useState(false);

  return (
    <div style={{ marginBottom: 4 }}>
      <div
        onClick={() => setOpen(!open)}
        style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer', userSelect: 'none' }}
      >
        <span style={{ fontSize: 9, color: '#bbb', width: 10 }}>{open ? '\u25BC' : '\u25B6'}</span>
        <span style={{ fontSize: 12, color: '#444' }}>{scenario.title}</span>
      </div>
      {open && (
        <div style={{ paddingLeft: 16, fontSize: 11, color: '#666', marginTop: 4 }}>
          {scenario.given.length > 0 && (
            <div style={{ marginBottom: 3 }}>
              <span style={{ fontWeight: 600, color: '#888' }}>Given: </span>
              {scenario.given.map((g, i) => (
                <div key={i} style={{ paddingLeft: 8 }}>- {g}</div>
              ))}
            </div>
          )}
          {scenario.when.length > 0 && (
            <div style={{ marginBottom: 3 }}>
              <span style={{ fontWeight: 600, color: '#888' }}>When: </span>
              {scenario.when.map((w, i) => (
                <div key={i} style={{ paddingLeft: 8 }}>- {w}</div>
              ))}
            </div>
          )}
          {scenario.then.length > 0 && (
            <div>
              <span style={{ fontWeight: 600, color: '#888' }}>Then: </span>
              {scenario.then.map((t, i) => (
                <div key={i} style={{ paddingLeft: 8 }}>- {t}</div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
