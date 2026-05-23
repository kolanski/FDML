import { memo } from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';
import { NODE_COLORS, type NodeType } from '../../theme/colors';
import type { SpecNodeData } from '../../layout/specToGraph';
import type { Entity, Action, Feature, Constraint, Flow } from '../../api/types';

const TYPE_ICONS: Record<NodeType, string> = {
  entity: '\u25A0',
  action: '\u25B6',
  feature: '\u2605',
  constraint: '\u26A0',
  flow: '\u2192',
  system: '\u2302',
  contour: '\u25CB',
  integration: '\u21C4',
  cross_flow: '\u21D2',
  shared_entity: '\u229E',
};

function SpecNode({ data }: NodeProps) {
  const d = data as unknown as SpecNodeData;
  const colors = NODE_COLORS[d.nodeType];

  return (
    <div
      style={{
        background: colors.bg,
        border: `2px solid ${colors.border}`,
        borderRadius: 8,
        padding: 0,
        minWidth: d.expanded ? 320 : 200,
        maxWidth: d.expanded ? 400 : 220,
        fontSize: 12,
        fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
        cursor: 'pointer',
        boxShadow: '0 1px 4px rgba(0,0,0,0.1)',
      }}
    >
      <Handle type="target" position={Position.Top} style={{ background: colors.border }} />

      {/* Header */}
      <div
        style={{
          padding: '6px 10px',
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          borderBottom: d.expanded ? `1px solid ${colors.border}40` : 'none',
        }}
      >
        <span
          style={{
            background: colors.badge,
            color: '#fff',
            padding: '1px 6px',
            borderRadius: 4,
            fontSize: 10,
            fontWeight: 600,
            textTransform: 'uppercase',
            letterSpacing: 0.5,
          }}
        >
          {TYPE_ICONS[d.nodeType]} {d.nodeType}
        </span>
        <span style={{ color: '#666', fontSize: 10, marginLeft: 'auto' }}>{d.fdmlId}</span>
      </div>

      {/* Name */}
      <div style={{ padding: '4px 10px', fontWeight: 600, fontSize: 13 }}>{d.label}</div>

      {/* Collapsed counts */}
      {!d.expanded && Object.keys(d.counts).length > 0 && (
        <div style={{ padding: '2px 10px 6px', color: '#888', fontSize: 10 }}>
          {Object.entries(d.counts)
            .map(([k, v]) => `${v} ${k}`)
            .join('  ')}
        </div>
      )}

      {/* Expanded content */}
      {d.expanded && <ExpandedContent data={d} />}

      <Handle type="source" position={Position.Bottom} style={{ background: colors.border }} />
    </div>
  );
}

function ExpandedContent({ data }: { data: SpecNodeData }) {
  switch (data.nodeType) {
    case 'entity':
      return <EntityExpanded entity={data.spec as Entity} />;
    case 'action':
      return <ActionExpanded action={data.spec as Action} />;
    case 'feature':
      return <FeatureExpanded feature={data.spec as Feature} />;
    case 'constraint':
      return <ConstraintExpanded constraint={data.spec as Constraint} />;
    case 'flow':
      return <FlowExpanded flow={data.spec as Flow} />;
    default:
      return null;
  }
}

function EntityExpanded({ entity }: { entity: Entity }) {
  return (
    <div style={{ padding: '4px 10px 8px' }}>
      {entity.description && (
        <div style={{ color: '#666', fontSize: 11, marginBottom: 6, fontStyle: 'italic' }}>
          {entity.description}
        </div>
      )}
      <div style={{ fontWeight: 600, fontSize: 10, color: '#555', marginBottom: 4 }}>FIELDS</div>
      <table style={{ width: '100%', fontSize: 11, borderCollapse: 'collapse' }}>
        <tbody>
          {entity.fields.map((f) => (
            <tr key={f.name} style={{ borderBottom: '1px solid #e5e7eb' }}>
              <td style={{ padding: '2px 4px', fontWeight: 500 }}>{f.name}</td>
              <td style={{ padding: '2px 4px', color: '#666' }}>{f.type}</td>
              <td style={{ padding: '2px 4px', color: f.required ? '#dc2626' : '#999', fontSize: 10 }}>
                {f.required ? 'req' : 'opt'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {entity.relationships && entity.relationships.length > 0 && (
        <>
          <div style={{ fontWeight: 600, fontSize: 10, color: '#555', marginTop: 8, marginBottom: 4 }}>
            RELATIONSHIPS
          </div>
          {entity.relationships.map((r) => (
            <div key={`${r.entity}-${r.type}`} style={{ fontSize: 11, color: '#555' }}>
              {r.type} {r.entity}
            </div>
          ))}
        </>
      )}
    </div>
  );
}

function ActionExpanded({ action }: { action: Action }) {
  return (
    <div style={{ padding: '4px 10px 8px' }}>
      {action.description && (
        <div style={{ color: '#666', fontSize: 11, marginBottom: 6, fontStyle: 'italic' }}>
          {action.description}
        </div>
      )}
      {action.input && (
        <div style={{ marginBottom: 4 }}>
          <span style={{ fontWeight: 600, fontSize: 10, color: '#555' }}>INPUT: </span>
          <span style={{ fontSize: 11 }}>
            {action.input.entity || ''} {action.input.fields?.join(', ')}
          </span>
        </div>
      )}
      {action.output && (
        <div style={{ marginBottom: 4 }}>
          <span style={{ fontWeight: 600, fontSize: 10, color: '#555' }}>OUTPUT: </span>
          <span style={{ fontSize: 11 }}>
            {action.output.entity || ''} {action.output.fields?.join(', ')}
          </span>
        </div>
      )}
      {action.preconditions && action.preconditions.length > 0 && (
        <div style={{ marginBottom: 4 }}>
          <div style={{ fontWeight: 600, fontSize: 10, color: '#555' }}>PRECONDITIONS</div>
          {action.preconditions.map((p, i) => (
            <div key={i} style={{ fontSize: 11, color: '#555', paddingLeft: 8 }}>
              - {p}
            </div>
          ))}
        </div>
      )}
      {action.postconditions && action.postconditions.length > 0 && (
        <div>
          <div style={{ fontWeight: 600, fontSize: 10, color: '#555' }}>POSTCONDITIONS</div>
          {action.postconditions.map((p, i) => (
            <div key={i} style={{ fontSize: 11, color: '#555', paddingLeft: 8 }}>
              - {p}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function FeatureExpanded({ feature }: { feature: Feature }) {
  return (
    <div style={{ padding: '4px 10px 8px' }}>
      {feature.description && (
        <div style={{ color: '#666', fontSize: 11, marginBottom: 6, fontStyle: 'italic' }}>
          {feature.description}
        </div>
      )}
      {feature.scenarios.map((s) => (
        <div key={s.id} style={{ marginBottom: 6 }}>
          <div style={{ fontWeight: 600, fontSize: 11, marginBottom: 2 }}>{s.title}</div>
          {s.given.map((g, i) => (
            <div key={`g${i}`} style={{ fontSize: 10, color: '#555', paddingLeft: 8 }}>
              <b>Given</b> {g}
            </div>
          ))}
          {s.when.map((w, i) => (
            <div key={`w${i}`} style={{ fontSize: 10, color: '#555', paddingLeft: 8 }}>
              <b>When</b> {w}
            </div>
          ))}
          {s.then.map((t, i) => (
            <div key={`t${i}`} style={{ fontSize: 10, color: '#555', paddingLeft: 8 }}>
              <b>Then</b> {t}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function ConstraintExpanded({ constraint }: { constraint: Constraint }) {
  return (
    <div style={{ padding: '4px 10px 8px' }}>
      {constraint.description && (
        <div style={{ color: '#666', fontSize: 11, marginBottom: 4, fontStyle: 'italic' }}>
          {constraint.description}
        </div>
      )}
      <div style={{ fontSize: 11 }}>
        <span style={{ fontWeight: 600, color: '#555', fontSize: 10 }}>TYPE: </span>
        {constraint.type}
      </div>
      <div style={{ fontSize: 11, marginTop: 2 }}>
        <span style={{ fontWeight: 600, color: '#555', fontSize: 10 }}>RULE: </span>
        {constraint.rule}
      </div>
      {constraint.entities && (
        <div style={{ fontSize: 11, marginTop: 2, color: '#666' }}>
          Applies to: {constraint.entities.join(', ')}
        </div>
      )}
    </div>
  );
}

function FlowExpanded({ flow }: { flow: Flow }) {
  return (
    <div style={{ padding: '4px 10px 8px' }}>
      {flow.description && (
        <div style={{ color: '#666', fontSize: 11, marginBottom: 6, fontStyle: 'italic' }}>
          {flow.description}
        </div>
      )}
      <div style={{ fontWeight: 600, fontSize: 10, color: '#555', marginBottom: 4 }}>STEPS</div>
      {flow.steps.map((s, i) => (
        <div key={s.id} style={{ fontSize: 11, color: '#555', marginBottom: 2 }}>
          {i + 1}. <b>{s.action}</b>
          {s.description && <span style={{ color: '#888' }}> - {s.description}</span>}
        </div>
      ))}
    </div>
  );
}

export default memo(SpecNode);
