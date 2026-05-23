import { useMemo, useCallback, useState } from 'react';
import type { FdmlDocument } from '../api/types';
import { buildHierarchy, type Hierarchy } from '../layout/buildHierarchy';
import type { InferredFlow } from '../layout/inferFlows';
import { NODE_COLORS } from '../theme/colors';
import FeatureCard from './FeatureCard';
import ActionCard from './ActionCard';
import EntityCard from './EntityCard';
import ConstraintCard from './ConstraintCard';

interface SpecViewProps {
  spec: FdmlDocument;
  searchQuery?: string;
  hierarchy: Hierarchy;
}

export default function SpecView({ spec, searchQuery, hierarchy }: SpecViewProps) {
  const q = (searchQuery || '').toLowerCase();

  const scrollToElement = useCallback((type: string, id: string) => {
    const el = document.getElementById(`${type}-${id}`);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
      const color = type === 'entity' ? NODE_COLORS.entity.border
        : type === 'action' ? NODE_COLORS.action.border
        : type === 'feature' ? NODE_COLORS.feature.border
        : NODE_COLORS.flow.border;
      el.style.boxShadow = `0 0 0 2px ${color}`;
      setTimeout(() => {
        el.style.boxShadow = '0 1px 3px rgba(0,0,0,0.06)';
      }, 1500);
    }
  }, []);

  const scrollToEntity = useCallback((entityId: string) => scrollToElement('entity', entityId), [scrollToElement]);
  const scrollToAction = useCallback((actionId: string) => scrollToElement('action', actionId), [scrollToElement]);

  // Filter helpers
  const matchesQuery = useCallback(
    (text: string) => !q || text.toLowerCase().includes(q),
    [q],
  );

  const filteredFeatures = useMemo(
    () =>
      q
        ? hierarchy.features.filter(
            (fn) =>
              matchesQuery(fn.feature.title) ||
              matchesQuery(fn.feature.description || '') ||
              fn.actions.some((a) => matchesQuery(a.action.name || '') || matchesQuery(a.action.id)),
          )
        : hierarchy.features,
    [hierarchy.features, q, matchesQuery],
  );

  const filteredOrphanActions = useMemo(
    () =>
      q
        ? hierarchy.orphanActions.filter(
            (an) => matchesQuery(an.action.name || '') || matchesQuery(an.action.id),
          )
        : hierarchy.orphanActions,
    [hierarchy.orphanActions, q, matchesQuery],
  );

  const filteredEntities = useMemo(
    () =>
      q
        ? hierarchy.entities.filter(
            (eu) => matchesQuery(eu.entity.name || '') || matchesQuery(eu.entity.id),
          )
        : hierarchy.entities,
    [hierarchy.entities, q, matchesQuery],
  );

  const filteredConstraints = useMemo(
    () =>
      q
        ? hierarchy.constraints.filter(
            (c) => matchesQuery(c.name) || matchesQuery(c.id),
          )
        : hierarchy.constraints,
    [hierarchy.constraints, q, matchesQuery],
  );

  return (
    <div
      style={{
        flex: 1,
        overflow: 'auto',
        padding: '20px 24px',
        background: '#f8f9fa',
      }}
    >
      <div style={{ maxWidth: 900, margin: '0 auto' }}>
        {/* Features */}
        {filteredFeatures.length > 0 && (
          <section style={{ marginBottom: 28 }}>
            <SectionDivider
              label="Features"
              color={NODE_COLORS.feature.badge}
              count={filteredFeatures.length}
            />
            {filteredFeatures.map((fn) => (
              <FeatureCard
                key={fn.feature.id}
                node={fn}
                onEntityClick={scrollToEntity}
                onActionClick={scrollToAction}
                defaultExpanded={!!q}
              />
            ))}
          </section>
        )}

        {/* Orphan Actions */}
        {filteredOrphanActions.length > 0 && (
          <section style={{ marginBottom: 28 }}>
            <SectionDivider
              label="Other Actions"
              color={NODE_COLORS.action.badge}
              count={filteredOrphanActions.length}
            />
            {filteredOrphanActions.map((an) => (
              <ActionCard
                key={an.action.id}
                node={an}
                onEntityClick={scrollToEntity}
              />
            ))}
          </section>
        )}

        {/* Inferred Flows */}
        {hierarchy.inferredFlows.length > 0 && (
          <section style={{ marginBottom: 28 }}>
            <SectionDivider
              label="Inferred Flows"
              color={NODE_COLORS.flow.badge}
              count={hierarchy.inferredFlows.length}
            />
            {hierarchy.inferredFlows.map((flow) => (
              <InferredFlowCard
                key={flow.id}
                flow={flow}
                actionMap={spec.actions}
                entityMap={spec.entities}
                onActionClick={scrollToAction}
                onEntityClick={scrollToEntity}
              />
            ))}
          </section>
        )}

        {/* Explicit Flows */}
        {hierarchy.flows.length > 0 && (
          <section style={{ marginBottom: 28 }}>
            <SectionDivider
              label="Flows"
              color={NODE_COLORS.flow.badge}
              count={hierarchy.flows.length}
            />
            {hierarchy.flows.map((fn) => (
              <FlowCard key={fn.flow.id} node={fn} onActionClick={scrollToAction} />
            ))}
          </section>
        )}

        {/* Entities */}
        {filteredEntities.length > 0 && (
          <section style={{ marginBottom: 28 }}>
            <SectionDivider
              label="Entities"
              color={NODE_COLORS.entity.badge}
              count={filteredEntities.length}
            />
            {filteredEntities.map((eu) => (
              <EntityCard key={eu.entity.id} usage={eu} onActionClick={scrollToAction} />
            ))}
          </section>
        )}

        {/* Constraints */}
        {filteredConstraints.length > 0 && (
          <section style={{ marginBottom: 28 }}>
            <SectionDivider
              label="Constraints"
              color={NODE_COLORS.constraint.badge}
              count={filteredConstraints.length}
            />
            {filteredConstraints.map((c) => (
              <ConstraintCard key={c.id} constraint={c} />
            ))}
          </section>
        )}
      </div>
    </div>
  );
}

function SectionDivider({ label, color, count }: { label: string; color: string; count: number }) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 10,
        marginBottom: 12,
      }}
    >
      <div style={{ height: 1, flex: 1, background: '#e5e7eb' }} />
      <span style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: 1, color }}>
        {label}
      </span>
      <span style={{ fontSize: 11, color: '#999' }}>({count})</span>
      <div style={{ height: 1, flex: 1, background: '#e5e7eb' }} />
    </div>
  );
}

function FlowCard({ node, onActionClick }: {
  node: { flow: { id: string; name: string; description?: string; steps: Array<{ id: string; action: string; description?: string }> }; actions: Array<{ id: string; name?: string }> };
  onActionClick?: (actionId: string) => void;
}) {
  const { flow } = node;

  return (
    <div
      style={{
        borderLeft: `3px solid ${NODE_COLORS.flow.border}`,
        background: '#fff',
        borderRadius: 6,
        marginBottom: 8,
        padding: '10px 14px',
        boxShadow: '0 1px 3px rgba(0,0,0,0.06)',
      }}
    >
      <div style={{ fontWeight: 600, fontSize: 13, color: '#333' }}>{flow.name}</div>
      {flow.description && (
        <div style={{ fontSize: 12, color: '#666', marginTop: 2 }}>{flow.description}</div>
      )}
      <div style={{ marginTop: 6 }}>
        {flow.steps.map((step, i) => {
          const action = node.actions[i];
          return (
            <div key={step.id} style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, padding: '2px 0' }}>
              <span style={{ color: NODE_COLORS.flow.badge, fontWeight: 600, minWidth: 14 }}>{i + 1}.</span>
              <span
                onClick={() => onActionClick?.(step.action)}
                style={{ fontFamily: 'monospace', color: '#333', cursor: onActionClick ? 'pointer' : 'default' }}
              >
                {action?.name || step.action}
              </span>
              {step.description && <span style={{ color: '#999' }}>— {step.description}</span>}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function InferredFlowCard({
  flow,
  actionMap,
  entityMap,
  onActionClick,
  onEntityClick,
}: {
  flow: InferredFlow;
  actionMap: Array<{ id: string; name?: string }>;
  entityMap: Array<{ id: string; name?: string }>;
  onActionClick?: (actionId: string) => void;
  onEntityClick?: (entityId: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const actions = new Map(actionMap.map((a) => [a.id, a]));
  const entities = new Map(entityMap.map((e) => [e.id, e]));

  const actionName = (id: string) => actions.get(id)?.name || id;
  const entityName = (id: string) => entities.get(id)?.name || id;

  // Build arrow chains: from root, follow edges
  const chains: Array<{ from: string; to: string; via: string }[]> = [];
  const rootSet = new Set(flow.rootActions);

  for (const root of flow.rootActions) {
    const chain: Array<{ from: string; to: string; via: string }> = [];
    for (const e of flow.edges) {
      if (e.fromAction === root) {
        chain.push({ from: e.fromAction, to: e.toAction, via: e.viaEntity });
      }
    }
    if (chain.length > 0) chains.push(chain);
  }

  return (
    <div
      style={{
        borderLeft: `3px solid ${NODE_COLORS.flow.border}`,
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
        <span style={{ fontWeight: 600, fontSize: 13, color: '#333' }}>{flow.name}</span>
        <span
          style={{
            fontSize: 9,
            background: NODE_COLORS.flow.bg,
            color: NODE_COLORS.flow.badge,
            padding: '1px 6px',
            borderRadius: 3,
            fontWeight: 500,
          }}
        >
          inferred
        </span>
        <span style={{ fontSize: 10, color: '#888' }}>
          {flow.actions.length} actions, {flow.entities.length} entities
        </span>
      </div>

      {/* Arrow chain preview (always visible) */}
      <div style={{ padding: '0 14px 8px 32px' }}>
        {chains.map((chain, ci) => (
          <div key={ci} style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 4, fontSize: 11, marginBottom: 2 }}>
            <span
              onClick={(e) => { e.stopPropagation(); onActionClick?.(chain[0].from); }}
              style={{ fontFamily: 'monospace', fontWeight: 600, color: NODE_COLORS.action.badge, cursor: 'pointer' }}
            >
              {actionName(chain[0].from)}
            </span>
            {chain.map((edge, i) => (
              <span key={i} style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                <span style={{ color: '#ccc' }}>--[</span>
                <span
                  onClick={(e) => { e.stopPropagation(); onEntityClick?.(edge.via); }}
                  style={{ color: NODE_COLORS.entity.badge, cursor: 'pointer', fontFamily: 'monospace', fontSize: 10 }}
                >
                  {entityName(edge.via)}
                </span>
                <span style={{ color: '#ccc' }}>]--&gt;</span>
                <span
                  onClick={(e) => { e.stopPropagation(); onActionClick?.(edge.to); }}
                  style={{ fontFamily: 'monospace', color: NODE_COLORS.action.badge, cursor: 'pointer' }}
                >
                  {actionName(edge.to)}
                </span>
              </span>
            ))}
          </div>
        ))}
      </div>

      {/* Expanded: all actions and entities */}
      {expanded && (
        <div style={{ padding: '0 14px 12px 32px', fontSize: 11, borderTop: '1px solid #f3f4f6' }}>
          <div style={{ marginTop: 8, marginBottom: 6 }}>
            <span style={{ fontWeight: 600, color: '#888', fontSize: 10, textTransform: 'uppercase' }}>All Actions:</span>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 4 }}>
              {flow.actions.map((aid) => (
                <span
                  key={aid}
                  onClick={() => onActionClick?.(aid)}
                  style={{
                    fontSize: 10,
                    background: rootSet.has(aid) ? NODE_COLORS.action.border : NODE_COLORS.action.bg,
                    color: rootSet.has(aid) ? '#fff' : NODE_COLORS.action.badge,
                    padding: '2px 8px',
                    borderRadius: 3,
                    cursor: 'pointer',
                    fontWeight: 500,
                    fontFamily: 'monospace',
                  }}
                >
                  {actionName(aid)}
                  {rootSet.has(aid) && ' (root)'}
                </span>
              ))}
            </div>
          </div>
          <div style={{ marginTop: 6 }}>
            <span style={{ fontWeight: 600, color: '#888', fontSize: 10, textTransform: 'uppercase' }}>Entities:</span>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 4 }}>
              {flow.entities.map((eid) => (
                <span
                  key={eid}
                  onClick={() => onEntityClick?.(eid)}
                  style={{
                    fontSize: 10,
                    background: NODE_COLORS.entity.bg,
                    color: NODE_COLORS.entity.badge,
                    padding: '2px 8px',
                    borderRadius: 3,
                    cursor: 'pointer',
                    fontWeight: 500,
                  }}
                >
                  {entityName(eid)}
                </span>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
