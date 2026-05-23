import { useMemo, useState } from 'react';
import type { FdmlDocument, SystemEntry, Integration, CrossFlow, SharedEntity } from '../api/types';
import { NODE_COLORS, SYSTEM_TYPE_ICONS } from '../theme/colors';

interface ArchitectureViewProps {
  spec: FdmlDocument;
  onDrillDown?: (systemId: string) => void;
}

export default function ArchitectureView({ spec, onDrillDown }: ArchitectureViewProps) {
  const [activeTab, setActiveTab] = useState<'landscape' | 'c4_diagram' | 'cross_flows' | 'shared_entities'>('landscape');

  const contourMap = useMemo(() => {
    const map = new Map<string, typeof spec.contours[0]>();
    for (const c of spec.contours) map.set(c.id, c);
    return map;
  }, [spec.contours]);

  const systemsByContour = useMemo(() => {
    const grouped = new Map<string, SystemEntry[]>();
    const ungrouped: SystemEntry[] = [];
    for (const sys of spec.systems) {
      if (sys.contour) {
        const list = grouped.get(sys.contour) || [];
        list.push(sys);
        grouped.set(sys.contour, list);
      } else {
        ungrouped.push(sys);
      }
    }
    return { grouped, ungrouped };
  }, [spec.systems]);

  const tabs: { id: typeof activeTab; label: string; count: number }[] = [
    { id: 'landscape', label: 'System Landscape', count: spec.systems.length },
    { id: 'c4_diagram', label: 'C4 Diagram', count: spec.systems.length },
    { id: 'cross_flows', label: 'Cross-Flows', count: spec.cross_flows.length },
    { id: 'shared_entities', label: 'Shared Entities', count: spec.shared_entities.length },
  ];

  return (
    <div style={{ flex: 1, overflow: 'auto', padding: '20px 24px', background: '#f8f9fa' }}>
      <div style={{ maxWidth: 1000, margin: '0 auto' }}>
        {/* Sub-tabs */}
        <div style={{ display: 'flex', gap: 4, marginBottom: 20, borderBottom: '1px solid #e5e7eb', paddingBottom: 8 }}>
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              style={{
                fontSize: 12,
                fontWeight: activeTab === tab.id ? 700 : 400,
                color: activeTab === tab.id ? '#1f2937' : '#888',
                background: activeTab === tab.id ? '#f3f4f6' : 'transparent',
                border: 'none',
                borderBottom: activeTab === tab.id ? '2px solid #1f2937' : '2px solid transparent',
                padding: '6px 12px',
                cursor: 'pointer',
                borderRadius: '4px 4px 0 0',
              }}
            >
              {tab.label}
              {tab.count > 0 && (
                <span style={{ fontSize: 10, color: '#999', marginLeft: 4 }}>({tab.count})</span>
              )}
            </button>
          ))}
        </div>

        {activeTab === 'landscape' && (
          <LandscapeView
            spec={spec}
            contourMap={contourMap}
            systemsByContour={systemsByContour}
            onDrillDown={onDrillDown}
          />
        )}
        {activeTab === 'c4_diagram' && (
          <C4DiagramView spec={spec} contourMap={contourMap} onDrillDown={onDrillDown} />
        )}
        {activeTab === 'cross_flows' && (
          <CrossFlowsView crossFlows={spec.cross_flows} systems={spec.systems} integrations={spec.integrations} />
        )}
        {activeTab === 'shared_entities' && (
          <SharedEntitiesView sharedEntities={spec.shared_entities} systems={spec.systems} />
        )}
      </div>
    </div>
  );
}

// --- Landscape View ---

function LandscapeView({
  spec,
  contourMap,
  systemsByContour,
  onDrillDown,
}: {
  spec: FdmlDocument;
  contourMap: Map<string, typeof spec.contours[0]>;
  systemsByContour: { grouped: Map<string, SystemEntry[]>; ungrouped: SystemEntry[] };
  onDrillDown?: (systemId: string) => void;
}) {
  const systemMap = useMemo(() => {
    const m = new Map<string, SystemEntry>();
    for (const s of spec.systems) m.set(s.id, s);
    return m;
  }, [spec.systems]);

  return (
    <div>
      {/* Contour groups */}
      {Array.from(systemsByContour.grouped.entries()).map(([contourId, systems]) => {
        const contour = contourMap.get(contourId);
        return (
          <div
            key={contourId}
            style={{
              border: '2px dashed ' + NODE_COLORS.contour.border,
              borderRadius: 10,
              padding: 16,
              marginBottom: 16,
              background: '#fafaff',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
              <span style={{ fontWeight: 700, fontSize: 14, color: NODE_COLORS.contour.badge }}>
                {contour?.name || contourId}
              </span>
              {contour?.trust_level && (
                <TrustBadge level={contour.trust_level} />
              )}
              {contour?.description && (
                <span style={{ fontSize: 11, color: '#888' }}>{contour.description}</span>
              )}
            </div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12 }}>
              {systems.map((sys) => (
                <SystemCard key={sys.id} system={sys} onDrillDown={onDrillDown} />
              ))}
            </div>
          </div>
        );
      })}

      {/* Ungrouped systems */}
      {systemsByContour.ungrouped.length > 0 && (
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontWeight: 600, fontSize: 12, color: '#888', marginBottom: 8, textTransform: 'uppercase' }}>
            Ungrouped Systems
          </div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12 }}>
            {systemsByContour.ungrouped.map((sys) => (
              <SystemCard key={sys.id} system={sys} onDrillDown={onDrillDown} />
            ))}
          </div>
        </div>
      )}

      {/* Integrations */}
      {spec.integrations.length > 0 && (
        <div style={{ marginTop: 24 }}>
          <SectionHeader label="Integrations" color={NODE_COLORS.integration.badge} count={spec.integrations.length} />
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {spec.integrations.map((integ) => (
              <IntegrationRow key={integ.id} integration={integ} systemMap={systemMap} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function SystemCard({ system, onDrillDown }: { system: SystemEntry; onDrillDown?: (id: string) => void }) {
  const icon = SYSTEM_TYPE_ICONS[system.type] || '';

  return (
    <div
      style={{
        background: '#fff',
        border: '1px solid #e5e7eb',
        borderRadius: 8,
        padding: '12px 16px',
        minWidth: 180,
        maxWidth: 260,
        boxShadow: '0 1px 3px rgba(0,0,0,0.06)',
        cursor: onDrillDown ? 'pointer' : undefined,
        transition: 'box-shadow 0.15s',
      }}
      onClick={() => onDrillDown?.(system.id)}
      onMouseEnter={(e) => { if (onDrillDown) (e.currentTarget as HTMLElement).style.boxShadow = '0 2px 8px rgba(0,0,0,0.12)'; }}
      onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.boxShadow = '0 1px 3px rgba(0,0,0,0.06)'; }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
        <span style={{ fontSize: 16 }}>{icon}</span>
        <span style={{ fontWeight: 600, fontSize: 13, color: '#333' }}>{system.name}</span>
      </div>
      <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', marginBottom: 4 }}>
        <TypeBadge type={system.type} />
        {system.technology && (
          <span style={{ fontSize: 9, background: '#f3f4f6', color: '#555', padding: '1px 6px', borderRadius: 3 }}>
            {system.technology}
          </span>
        )}
        {system.owner && (
          <span style={{ fontSize: 9, background: '#ede9fe', color: '#7c3aed', padding: '1px 6px', borderRadius: 3 }}>
            {system.owner}
          </span>
        )}
      </div>
      {system.description && (
        <div style={{ fontSize: 11, color: '#666', marginTop: 4 }}>{system.description}</div>
      )}
      {system.spec && (
        <div style={{ fontSize: 10, color: '#0284c7', marginTop: 4, fontFamily: 'monospace', textDecoration: 'underline' }}>
          {system.spec}
        </div>
      )}
    </div>
  );
}

function IntegrationRow({
  integration,
  systemMap,
}: {
  integration: Integration;
  systemMap: Map<string, SystemEntry>;
}) {
  const fromSys = systemMap.get(integration.from);
  const toSys = systemMap.get(integration.to);
  const hasDetails = integration.endpoints.length > 0 || integration.channels.length > 0;
  const manyEndpoints = integration.endpoints.length > 4;
  const [collapsed, setCollapsed] = useState(manyEndpoints);

  const methodColor: Record<string, string> = {
    GET: '#16a34a', POST: '#2563eb', PUT: '#d97706', DELETE: '#dc2626',
    PATCH: '#7c3aed', PUBLISH: '#0d9488', SUBSCRIBE: '#0d9488',
  };

  return (
    <div
      style={{
        background: '#fff',
        border: '1px solid #e5e7eb',
        borderLeft: '3px solid ' + NODE_COLORS.integration.border,
        borderRadius: 6,
        padding: '8px 12px',
        boxShadow: '0 1px 2px rgba(0,0,0,0.04)',
      }}
    >
      {/* Header */}
      <div
        onClick={hasDetails ? () => setCollapsed(!collapsed) : undefined}
        style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: hasDetails ? 'pointer' : 'default', userSelect: 'none' }}
      >
        {hasDetails && (
          <span style={{ fontSize: 10, color: '#999', width: 12 }}>{collapsed ? '\u25B6' : '\u25BC'}</span>
        )}
        <span style={{ fontWeight: 600, fontSize: 12, color: '#333' }}>
          {fromSys?.name || integration.from}
        </span>
        <span style={{ color: NODE_COLORS.integration.badge, fontSize: 11 }}>{'\u2192'}</span>
        <span style={{ fontWeight: 600, fontSize: 12, color: '#333' }}>
          {toSys?.name || integration.to}
        </span>
        <span style={{
          fontSize: 9,
          background: NODE_COLORS.integration.bg,
          color: NODE_COLORS.integration.badge,
          padding: '1px 6px',
          borderRadius: 3,
          fontWeight: 500,
        }}>
          {integration.type}
        </span>
        {integration.protocol && (
          <span style={{ fontSize: 10, color: '#888' }}>{integration.protocol}</span>
        )}
        {integration.async && (
          <span style={{ fontSize: 9, background: '#fef3c7', color: '#92400e', padding: '1px 4px', borderRadius: 2 }}>async</span>
        )}
        {integration.endpoints.length > 0 && (
          <span style={{ fontSize: 9, color: '#999' }}>{integration.endpoints.length} endpoints</span>
        )}
        {integration.channels.length > 0 && (
          <span style={{ fontSize: 9, color: '#999' }}>{integration.channels.length} channels</span>
        )}
      </div>

      {integration.description && (
        <div style={{ fontSize: 11, color: '#666', marginTop: 2, marginLeft: hasDetails ? 20 : 0 }}>{integration.description}</div>
      )}

      {/* Endpoints */}
      {!collapsed && integration.endpoints.length > 0 && (
        <div style={{ marginTop: 6, marginLeft: hasDetails ? 20 : 0 }}>
          {integration.endpoints.map((ep, i) => (
            <div key={i} style={{ fontFamily: 'monospace', fontSize: 10, marginTop: 1, display: 'flex', gap: 6, alignItems: 'baseline' }}>
              <span style={{ color: methodColor[ep.method] || '#333', fontWeight: 700, minWidth: 42 }}>{ep.method}</span>
              <span style={{ color: '#333' }}>{ep.path}</span>
              {ep.description && <span style={{ color: '#999', fontFamily: 'system-ui, sans-serif', fontSize: 10 }}>{ep.description}</span>}
            </div>
          ))}
        </div>
      )}

      {/* Channels */}
      {!collapsed && integration.channels.length > 0 && (
        <div style={{ marginTop: 6, marginLeft: hasDetails ? 20 : 0, display: 'flex', gap: 4, flexWrap: 'wrap' }}>
          {integration.channels.map((ch) => (
            <span key={ch} style={{ fontFamily: 'monospace', fontSize: 9, background: '#f0fdf4', color: '#166534', padding: '1px 6px', borderRadius: 3, border: '1px solid #bbf7d0' }}>
              {ch}
            </span>
          ))}
        </div>
      )}

      {/* Data entities */}
      {!collapsed && integration.data_entities.length > 0 && (
        <div style={{ marginTop: 4, marginLeft: hasDetails ? 20 : 0, display: 'flex', gap: 4, flexWrap: 'wrap', alignItems: 'center' }}>
          <span style={{ fontSize: 9, color: '#999' }}>entities:</span>
          {integration.data_entities.map((e) => (
            <span key={e} style={{ fontFamily: 'monospace', fontSize: 9, background: NODE_COLORS.entity.bg, color: NODE_COLORS.entity.badge, padding: '1px 5px', borderRadius: 2 }}>
              {e}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

// --- C4 Diagram View ---

const C4_BOX_W = 160;
const C4_BOX_H = 90;
const C4_PAD = 24;
const C4_ROW_GAP = 100;
const C4_COL_GAP = 32;
const C4_BOUNDARY_PAD = 20;

interface C4Node {
  id: string;
  name: string;
  type: string;
  technology?: string;
  description?: string;
  contour?: string;
  x: number;
  y: number;
}

function C4DiagramView({
  spec,
  contourMap,
  onDrillDown,
}: {
  spec: FdmlDocument;
  contourMap: Map<string, typeof spec.contours[0]>;
  onDrillDown?: (systemId: string) => void;
}) {
  const layout = useMemo(() => {
    const trustOrder: Record<string, number> = { public: 0, internal: 1, restricted: 2, critical: 3 };
    const contourIds = Array.from(new Set(spec.systems.map((s) => s.contour || '__none__')));
    contourIds.sort((a, b) => {
      const ta = contourMap.get(a)?.trust_level || '';
      const tb = contourMap.get(b)?.trust_level || '';
      return (trustOrder[ta] ?? 99) - (trustOrder[tb] ?? 99);
    });

    const nodes: C4Node[] = [];
    const boundaries: { id: string; name: string; trust?: string; x: number; y: number; w: number; h: number }[] = [];
    let curY = C4_PAD;

    for (const contourId of contourIds) {
      const systems = spec.systems.filter((s) => (s.contour || '__none__') === contourId);
      if (systems.length === 0) continue;

      const cols = Math.min(systems.length, 4);
      const rows = Math.ceil(systems.length / cols);
      const boundaryW = cols * (C4_BOX_W + C4_COL_GAP) - C4_COL_GAP + C4_BOUNDARY_PAD * 2;
      const boundaryH = rows * (C4_BOX_H + C4_ROW_GAP) - C4_ROW_GAP + C4_BOUNDARY_PAD * 2 + 28;
      const boundaryX = C4_PAD;

      if (contourId !== '__none__') {
        const contour = contourMap.get(contourId);
        boundaries.push({
          id: contourId,
          name: contour?.name || contourId,
          trust: contour?.trust_level,
          x: boundaryX,
          y: curY,
          w: boundaryW,
          h: boundaryH,
        });
      }

      const startX = boundaryX + C4_BOUNDARY_PAD;
      const startY = curY + (contourId !== '__none__' ? 28 + C4_BOUNDARY_PAD : C4_BOUNDARY_PAD);

      systems.forEach((sys, i) => {
        const col = i % cols;
        const row = Math.floor(i / cols);
        nodes.push({
          id: sys.id,
          name: sys.name,
          type: sys.type,
          technology: sys.technology,
          description: sys.description,
          contour: sys.contour,
          x: startX + col * (C4_BOX_W + C4_COL_GAP),
          y: startY + row * (C4_BOX_H + C4_ROW_GAP),
        });
      });

      curY += boundaryH + 30;
    }

    const nodeMap = new Map(nodes.map((n) => [n.id, n]));
    const edges: { from: C4Node; to: C4Node; type: string; protocol?: string; label?: string }[] = [];
    for (const integ of spec.integrations) {
      const fromNode = nodeMap.get(integ.from);
      const toNode = nodeMap.get(integ.to);
      if (fromNode && toNode) {
        edges.push({
          from: fromNode,
          to: toNode,
          type: integ.type,
          protocol: integ.protocol,
          label: integ.description || `${integ.type}${integ.protocol ? ' / ' + integ.protocol : ''}`,
        });
      }
    }

    const totalW = Math.max(...nodes.map((n) => n.x + C4_BOX_W), ...boundaries.map((b) => b.x + b.w)) + C4_PAD;
    const totalH = curY + C4_PAD;

    return { nodes, edges, boundaries, totalW, totalH };
  }, [spec, contourMap]);

  const [hoveredNode, setHoveredNode] = useState<string | null>(null);

  const activeEdges = useMemo(() => {
    if (!hoveredNode) return null;
    return new Set(
      layout.edges
        .filter((e) => e.from.id === hoveredNode || e.to.id === hoveredNode)
        .map((_, i) => i)
    );
  }, [hoveredNode, layout.edges]);

  const trustColors: Record<string, { stroke: string; fill: string }> = {
    public: { stroke: '#22c55e', fill: '#f0fdf4' },
    internal: { stroke: '#3b82f6', fill: '#eff6ff' },
    restricted: { stroke: '#f59e0b', fill: '#fffbeb' },
    critical: { stroke: '#ef4444', fill: '#fef2f2' },
  };

  const typeColors: Record<string, string> = {
    frontend: '#3b82f6',
    gateway: '#8b5cf6',
    service: '#059669',
    worker: '#d97706',
    database: '#6b7280',
  };

  return (
    <div style={{ overflow: 'auto', flex: 1 }}>
      <svg
        width={layout.totalW}
        height={layout.totalH}
        style={{ minWidth: '100%', fontFamily: 'system-ui, -apple-system, sans-serif' }}
      >
        <defs>
          <marker id="c4-arrow" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
            <path d="M0,0 L8,3 L0,6" fill="#888" />
          </marker>
          <marker id="c4-arrow-active" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
            <path d="M0,0 L8,3 L0,6" fill="#2563eb" />
          </marker>
        </defs>

        {/* Boundaries */}
        {layout.boundaries.map((b) => {
          const tc = trustColors[b.trust || ''] || { stroke: '#94a3b8', fill: '#f8fafc' };
          return (
            <g key={b.id}>
              <rect x={b.x} y={b.y} width={b.w} height={b.h} rx={8}
                fill={tc.fill} stroke={tc.stroke} strokeWidth={2} strokeDasharray="8 4" fillOpacity={0.5} />
              <text x={b.x + 12} y={b.y + 18} fontSize={12} fontWeight={700} fill={tc.stroke}>
                {b.name}
              </text>
              {b.trust && (
                <text x={b.x + 12 + b.name.length * 7 + 8} y={b.y + 18} fontSize={10} fill={tc.stroke} opacity={0.7}>
                  [{b.trust}]
                </text>
              )}
            </g>
          );
        })}

        {/* Edges */}
        {layout.edges.map((edge, i) => {
          const isActive = activeEdges?.has(i);
          const dimmed = activeEdges !== null && !isActive;
          const fx = edge.from.x + C4_BOX_W / 2;
          const fy = edge.from.y + C4_BOX_H / 2;
          const tx = edge.to.x + C4_BOX_W / 2;
          const ty = edge.to.y + C4_BOX_H / 2;
          const angle = Math.atan2(ty - fy, tx - fx);
          const clipFrom = clipToBox(fx, fy, C4_BOX_W, C4_BOX_H, angle);
          const clipTo = clipToBox(tx, ty, C4_BOX_W, C4_BOX_H, angle + Math.PI);
          const mx = (clipFrom.x + clipTo.x) / 2;
          const my = (clipFrom.y + clipTo.y) / 2;
          const shortLabel = edge.type + (edge.protocol ? ` / ${edge.protocol}` : '');

          return (
            <g key={i} opacity={dimmed ? 0.15 : 1}>
              <line x1={clipFrom.x} y1={clipFrom.y} x2={clipTo.x} y2={clipTo.y}
                stroke={isActive ? '#2563eb' : '#94a3b8'} strokeWidth={isActive ? 2 : 1}
                markerEnd={isActive ? 'url(#c4-arrow-active)' : 'url(#c4-arrow)'} />
              <rect x={mx - shortLabel.length * 3} y={my - 8} width={shortLabel.length * 6 + 8} height={14}
                fill="white" rx={3} stroke={isActive ? '#2563eb' : '#d1d5db'} strokeWidth={0.5} />
              <text x={mx + 4} y={my + 3} fontSize={9} fill={isActive ? '#2563eb' : '#888'} textAnchor="middle">
                {shortLabel}
              </text>
            </g>
          );
        })}

        {/* System boxes */}
        {layout.nodes.map((node) => {
          const isHovered = hoveredNode === node.id;
          const dimmed = hoveredNode !== null && !isHovered &&
            !layout.edges.some((e) =>
              (e.from.id === hoveredNode && e.to.id === node.id) ||
              (e.to.id === hoveredNode && e.from.id === node.id)
            );
          const color = typeColors[node.type] || '#4b5563';

          return (
            <g key={node.id} style={{ cursor: onDrillDown ? 'pointer' : 'default' }}
              onClick={() => onDrillDown?.(node.id)}
              onMouseEnter={() => setHoveredNode(node.id)}
              onMouseLeave={() => setHoveredNode(null)}
              opacity={dimmed ? 0.25 : 1}>
              <rect x={node.x} y={node.y} width={C4_BOX_W} height={C4_BOX_H} rx={6}
                fill={isHovered ? color : 'white'} stroke={color} strokeWidth={isHovered ? 2.5 : 1.5}
                filter={isHovered ? 'drop-shadow(0 2px 6px rgba(0,0,0,0.15))' : undefined} />
              <text x={node.x + C4_BOX_W / 2} y={node.y + 24} textAnchor="middle"
                fontSize={12} fontWeight={700} fill={isHovered ? 'white' : '#1f2937'}>
                {node.name.length > 20 ? node.name.slice(0, 18) + '...' : node.name}
              </text>
              <text x={node.x + C4_BOX_W / 2} y={node.y + 40} textAnchor="middle"
                fontSize={9} fill={isHovered ? 'rgba(255,255,255,0.8)' : color} fontWeight={600}>
                [{node.type}]
              </text>
              {node.technology && (
                <text x={node.x + C4_BOX_W / 2} y={node.y + 55} textAnchor="middle"
                  fontSize={9} fill={isHovered ? 'rgba(255,255,255,0.7)' : '#888'} fontStyle="italic">
                  {node.technology.length > 22 ? node.technology.slice(0, 20) + '...' : node.technology}
                </text>
              )}
              {node.description && (
                <text x={node.x + C4_BOX_W / 2} y={node.y + 72} textAnchor="middle"
                  fontSize={8} fill={isHovered ? 'rgba(255,255,255,0.6)' : '#aaa'}>
                  {node.description.length > 28 ? node.description.slice(0, 26) + '...' : node.description}
                </text>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function clipToBox(cx: number, cy: number, w: number, h: number, angle: number) {
  const hw = w / 2;
  const hh = h / 2;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  const scaleX = cos !== 0 ? Math.abs(hw / cos) : Infinity;
  const scaleY = sin !== 0 ? Math.abs(hh / sin) : Infinity;
  const scale = Math.min(scaleX, scaleY);
  return { x: cx + cos * scale, y: cy + sin * scale };
}

// --- Cross-Flows View ---

function CrossFlowsView({
  crossFlows,
  systems,
  integrations,
}: {
  crossFlows: CrossFlow[];
  systems: SystemEntry[];
  integrations: Integration[];
}) {
  const systemMap = useMemo(() => {
    const m = new Map<string, SystemEntry>();
    for (const s of systems) m.set(s.id, s);
    return m;
  }, [systems]);

  const integrationMap = useMemo(() => {
    const m = new Map<string, Integration>();
    for (const i of integrations) m.set(i.id, i);
    return m;
  }, [integrations]);

  if (crossFlows.length === 0) {
    return <div style={{ color: '#888', fontSize: 13 }}>No cross-flows defined.</div>;
  }

  return (
    <div>
      {crossFlows.map((cf) => (
        <CrossFlowCard key={cf.id} crossFlow={cf} systemMap={systemMap} integrationMap={integrationMap} />
      ))}
    </div>
  );
}

function CrossFlowCard({
  crossFlow,
  systemMap,
  integrationMap,
}: {
  crossFlow: CrossFlow;
  systemMap: Map<string, SystemEntry>;
  integrationMap: Map<string, Integration>;
}) {
  const systemOrder = useMemo(() => {
    const order: string[] = [];
    for (const step of crossFlow.steps) {
      if (!order.includes(step.system)) order.push(step.system);
    }
    return order;
  }, [crossFlow.steps]);

  return (
    <div
      style={{
        background: '#fff',
        border: '1px solid #e5e7eb',
        borderLeft: '3px solid ' + NODE_COLORS.cross_flow.border,
        borderRadius: 8,
        marginBottom: 12,
        padding: 16,
        boxShadow: '0 1px 3px rgba(0,0,0,0.06)',
      }}
    >
      <div style={{ fontWeight: 700, fontSize: 14, color: '#333', marginBottom: 2 }}>{crossFlow.name}</div>
      {crossFlow.description && (
        <div style={{ fontSize: 12, color: '#666', marginBottom: 4 }}>{crossFlow.description}</div>
      )}
      {crossFlow.trigger && (
        <div style={{ fontSize: 11, color: '#888', marginBottom: 8 }}>
          <strong>Trigger:</strong> {crossFlow.trigger}
        </div>
      )}

      {/* Swimlane diagram */}
      <div style={{ marginTop: 8, overflow: 'auto' }}>
        {systemOrder.map((sysId) => {
          const sys = systemMap.get(sysId);
          const stepsInSystem = crossFlow.steps.filter((s) => s.system === sysId);
          return (
            <div key={sysId} style={{ display: 'flex', alignItems: 'center', borderBottom: '1px solid #f3f4f6', padding: '6px 0', gap: 8 }}>
              <div style={{ minWidth: 120, fontSize: 11, fontWeight: 600, color: '#555' }}>
                {SYSTEM_TYPE_ICONS[sys?.type || ''] || ''} {sys?.name || sysId}
              </div>
              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                {stepsInSystem.map((step) => {
                  return (
                    <div key={step.id} style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                      <div
                        style={{
                          background: NODE_COLORS.cross_flow.bg,
                          border: '1px solid ' + NODE_COLORS.cross_flow.border,
                          borderRadius: 6,
                          padding: '4px 10px',
                          fontSize: 11,
                        }}
                      >
                        <span style={{ fontWeight: 600, color: '#333' }}>{step.id}</span>
                        {step.description && (
                          <span style={{ color: '#666', marginLeft: 4 }}>{step.description}</span>
                        )}
                      </div>
                      {step.on_success && (
                        <span style={{ color: NODE_COLORS.cross_flow.badge, fontSize: 10 }}>{'\u2192'}</span>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// --- Shared Entities View ---

function SharedEntitiesView({
  sharedEntities,
  systems,
}: {
  sharedEntities: SharedEntity[];
  systems: SystemEntry[];
}) {
  const systemMap = useMemo(() => {
    const m = new Map<string, SystemEntry>();
    for (const s of systems) m.set(s.id, s);
    return m;
  }, [systems]);

  if (sharedEntities.length === 0) {
    return <div style={{ color: '#888', fontSize: 13 }}>No shared entities defined.</div>;
  }

  return (
    <div>
      {/* Matrix view */}
      <div style={{ overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead>
            <tr>
              <th style={thStyle}>Entity</th>
              <th style={thStyle}>Canonical</th>
              {systems.map((s) => (
                <th key={s.id} style={thStyle}>{s.name}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sharedEntities.map((se) => (
              <tr key={se.entity}>
                <td style={tdStyle}>
                  <span style={{ fontWeight: 600 }}>{se.entity}</span>
                  {se.description && (
                    <div style={{ fontSize: 10, color: '#888' }}>{se.description}</div>
                  )}
                </td>
                <td style={tdStyle}>
                  {se.canonical_system && (
                    <span style={{ fontSize: 10, color: '#555' }}>
                      {systemMap.get(se.canonical_system)?.name || se.canonical_system}
                    </span>
                  )}
                </td>
                {systems.map((s) => {
                  const ctx = se.contexts.find((c) => c.system === s.id);
                  return (
                    <td key={s.id} style={tdStyle}>
                      {ctx ? (
                        <div>
                          {ctx.role && <RoleBadge role={ctx.role} />}
                          <div style={{ fontSize: 10, color: '#666' }}>
                            {ctx.fields.length} fields
                          </div>
                        </div>
                      ) : (
                        <span style={{ color: '#ddd' }}>-</span>
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Detail cards */}
      <div style={{ marginTop: 20 }}>
        {sharedEntities.map((se) => (
          <div
            key={se.entity}
            style={{
              background: '#fff',
              border: '1px solid #e5e7eb',
              borderLeft: '3px solid ' + NODE_COLORS.shared_entity.border,
              borderRadius: 8,
              marginBottom: 10,
              padding: 14,
              boxShadow: '0 1px 3px rgba(0,0,0,0.06)',
            }}
          >
            <div style={{ fontWeight: 700, fontSize: 14, color: '#333' }}>{se.entity}</div>
            {se.description && (
              <div style={{ fontSize: 12, color: '#666', marginBottom: 6 }}>{se.description}</div>
            )}
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 8 }}>
              {se.contexts.map((ctx) => {
                const sys = systemMap.get(ctx.system);
                return (
                  <div
                    key={ctx.system}
                    style={{
                      background: '#f8f9fa',
                      border: '1px solid #e5e7eb',
                      borderRadius: 6,
                      padding: '8px 12px',
                      minWidth: 160,
                    }}
                  >
                    <div style={{ fontWeight: 600, fontSize: 11, color: '#555' }}>
                      {sys?.name || ctx.system}
                    </div>
                    <div style={{ fontSize: 10, color: '#888', fontFamily: 'monospace' }}>
                      {ctx.entity_id}
                    </div>
                    {ctx.role && <RoleBadge role={ctx.role} />}
                    {ctx.fields.length > 0 && (
                      <div style={{ marginTop: 4, display: 'flex', flexWrap: 'wrap', gap: 2 }}>
                        {ctx.fields.map((f) => (
                          <span key={f} style={{ fontSize: 9, background: '#e0f2fe', color: '#0284c7', padding: '1px 4px', borderRadius: 2, fontFamily: 'monospace' }}>
                            {f}
                          </span>
                        ))}
                      </div>
                    )}
                    {ctx.notes && (
                      <div style={{ fontSize: 10, color: '#888', marginTop: 4, fontStyle: 'italic' }}>{ctx.notes}</div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// --- Shared Components ---

const thStyle: React.CSSProperties = {
  textAlign: 'left',
  padding: '6px 10px',
  borderBottom: '2px solid #e5e7eb',
  fontSize: 11,
  fontWeight: 700,
  color: '#555',
  whiteSpace: 'nowrap',
};

const tdStyle: React.CSSProperties = {
  padding: '6px 10px',
  borderBottom: '1px solid #f3f4f6',
  verticalAlign: 'top',
};

function TypeBadge({ type }: { type: string }) {
  return (
    <span style={{
      fontSize: 9,
      background: NODE_COLORS.system.bg,
      color: NODE_COLORS.system.badge,
      padding: '1px 6px',
      borderRadius: 3,
      fontWeight: 600,
      textTransform: 'uppercase',
    }}>
      {type}
    </span>
  );
}

function TrustBadge({ level }: { level: string }) {
  const colors: Record<string, { bg: string; fg: string }> = {
    public: { bg: '#dcfce7', fg: '#16a34a' },
    internal: { bg: '#dbeafe', fg: '#2563eb' },
    restricted: { bg: '#fef3c7', fg: '#d97706' },
    critical: { bg: '#fee2e2', fg: '#dc2626' },
  };
  const c = colors[level] || { bg: '#f3f4f6', fg: '#555' };
  return (
    <span style={{ fontSize: 9, background: c.bg, color: c.fg, padding: '1px 6px', borderRadius: 3, fontWeight: 600 }}>
      {level}
    </span>
  );
}

function RoleBadge({ role }: { role: string }) {
  const colors: Record<string, { bg: string; fg: string }> = {
    source: { bg: '#dcfce7', fg: '#16a34a' },
    replica: { bg: '#dbeafe', fg: '#2563eb' },
    projection: { bg: '#f3e8ff', fg: '#9333ea' },
    cache: { bg: '#fef3c7', fg: '#d97706' },
  };
  const c = colors[role] || { bg: '#f3f4f6', fg: '#555' };
  return (
    <span style={{ fontSize: 9, background: c.bg, color: c.fg, padding: '1px 5px', borderRadius: 2, fontWeight: 600, marginTop: 2, display: 'inline-block' }}>
      {role}
    </span>
  );
}

function SectionHeader({ label, color, count }: { label: string; color: string; count: number }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 12 }}>
      <div style={{ height: 1, flex: 1, background: '#e5e7eb' }} />
      <span style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: 1, color }}>
        {label}
      </span>
      <span style={{ fontSize: 11, color: '#999' }}>({count})</span>
      <div style={{ height: 1, flex: 1, background: '#e5e7eb' }} />
    </div>
  );
}
