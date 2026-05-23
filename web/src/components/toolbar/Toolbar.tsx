import { NODE_COLORS, type NodeType } from '../../theme/colors';
import type { ViewMode } from '../../App';

interface ToolbarProps {
  systemName?: string;
  counts: Record<string, number>;
  activeView: ViewMode;
  onViewChange: (view: ViewMode) => void;
  healthScore: number | null;
  onHealthClick: () => void;
  searchQuery: string;
  onSearchChange: (query: string) => void;
  hasArchitecture?: boolean;
}

const TYPE_ORDER: NodeType[] = ['feature', 'action', 'entity', 'constraint', 'flow'];

const VIEW_TABS: { id: ViewMode; label: string; archOnly?: boolean }[] = [
  { id: 'architecture', label: 'Architecture', archOnly: true },
  { id: 'spec', label: 'Spec' },
  { id: 'graph', label: 'Graph' },
  { id: 'flows', label: 'Flows' },
];

export default function Toolbar({
  systemName,
  counts,
  activeView,
  onViewChange,
  healthScore,
  onHealthClick,
  searchQuery,
  onSearchChange,
  hasArchitecture,
}: ToolbarProps) {
  const scoreColor =
    healthScore === null
      ? '#999'
      : healthScore > 80
        ? '#16a34a'
        : healthScore > 50
          ? '#d97706'
          : '#dc2626';

  return (
    <div
      style={{
        height: 44,
        background: '#fff',
        borderBottom: '1px solid #e5e7eb',
        display: 'flex',
        alignItems: 'center',
        padding: '0 16px',
        gap: 12,
        fontSize: 13,
        flexShrink: 0,
      }}
    >
      <span style={{ fontWeight: 700, fontSize: 14 }}>
        FDML {systemName && `- ${systemName}`}
      </span>
      <div style={{ width: 1, height: 20, background: '#e5e7eb' }} />
      {TYPE_ORDER.map((t) => {
        const c = counts[t] || 0;
        if (c === 0) return null;
        return (
          <span
            key={t}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 4,
              fontSize: 11,
            }}
          >
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: 2,
                background: NODE_COLORS[t].border,
                display: 'inline-block',
              }}
            />
            {c} {t}{c !== 1 ? 's' : ''}
          </span>
        );
      })}

      <div style={{ width: 1, height: 20, background: '#e5e7eb' }} />

      {/* View tabs */}
      <div style={{ display: 'flex', gap: 2 }}>
        {VIEW_TABS.filter((tab) => !tab.archOnly || hasArchitecture).map((tab) => (
          <button
            key={tab.id}
            onClick={() => onViewChange(tab.id)}
            style={{
              fontSize: 11,
              fontWeight: activeView === tab.id ? 700 : 400,
              color: activeView === tab.id ? '#1f2937' : '#888',
              background: activeView === tab.id ? '#f3f4f6' : 'transparent',
              border: 'none',
              borderBottom: activeView === tab.id ? '2px solid #1f2937' : '2px solid transparent',
              padding: '6px 10px',
              cursor: 'pointer',
              borderRadius: '4px 4px 0 0',
            }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div style={{ flex: 1 }} />

      {/* Search */}
      <input
        type="text"
        placeholder="Search..."
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        style={{
          fontSize: 11,
          padding: '4px 10px',
          border: '1px solid #d1d5db',
          borderRadius: 4,
          outline: 'none',
          width: 160,
          background: '#f9fafb',
        }}
      />

      {/* Health badge */}
      {healthScore !== null && (
        <button
          onClick={onHealthClick}
          title={`Health score: ${healthScore}`}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 4,
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            padding: '2px 6px',
            borderRadius: 4,
            fontSize: 11,
            fontWeight: 600,
            color: scoreColor,
          }}
        >
          <span
            style={{
              width: 10,
              height: 10,
              borderRadius: '50%',
              background: scoreColor,
              display: 'inline-block',
            }}
          />
          {healthScore}
        </button>
      )}
    </div>
  );
}
