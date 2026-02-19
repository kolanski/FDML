import type { SpecHealth, HealthIssue, Severity } from '../layout/specHealth';

interface HealthPanelProps {
  health: SpecHealth;
  onClose: () => void;
  onIssueClick?: (elementType: string, elementId: string) => void;
}

const SEVERITY_CONFIG: Record<Severity, { color: string; bg: string; label: string }> = {
  error: { color: '#dc2626', bg: '#fef2f2', label: 'Errors' },
  warning: { color: '#d97706', bg: '#fffbeb', label: 'Warnings' },
  info: { color: '#2563eb', bg: '#eff6ff', label: 'Info' },
};

export default function HealthPanel({ health, onClose, onIssueClick }: HealthPanelProps) {
  const scoreColor = health.score > 80 ? '#16a34a' : health.score > 50 ? '#d97706' : '#dc2626';

  const grouped = {
    error: health.issues.filter((i) => i.severity === 'error'),
    warning: health.issues.filter((i) => i.severity === 'warning'),
    info: health.issues.filter((i) => i.severity === 'info'),
  };

  return (
    <div
      style={{
        width: 320,
        background: '#fff',
        borderLeft: '1px solid #e5e7eb',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        flexShrink: 0,
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: '14px 16px',
          borderBottom: '1px solid #e5e7eb',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}
      >
        <span style={{ fontWeight: 700, fontSize: 14 }}>Spec Health</span>
        <button
          onClick={onClose}
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            fontSize: 18,
            color: '#999',
            padding: '0 4px',
          }}
        >
          x
        </button>
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: 16 }}>
        {/* Score */}
        <div style={{ textAlign: 'center', marginBottom: 20 }}>
          <div
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              width: 72,
              height: 72,
              borderRadius: '50%',
              border: `4px solid ${scoreColor}`,
              fontSize: 24,
              fontWeight: 700,
              color: scoreColor,
            }}
          >
            {health.score}
          </div>
          <div style={{ fontSize: 11, color: '#888', marginTop: 6 }}>Health Score</div>
        </div>

        {/* Coverage metrics */}
        <div style={{ marginBottom: 20 }}>
          <div style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: 0.5, color: '#888', marginBottom: 8 }}>
            Coverage
          </div>
          <CoverageBar
            label="Features with actions"
            value={health.coverage.featuresWithActions}
            total={health.coverage.totalFeatures}
          />
          <CoverageBar
            label="Entities used"
            value={health.coverage.entitiesUsed}
            total={health.coverage.totalEntities}
          />
          <CoverageBar
            label="Actions in features"
            value={health.coverage.actionsInFeatures}
            total={health.coverage.totalActions}
          />
          <CoverageBar
            label="Constraints linked"
            value={health.coverage.constraintsLinked}
            total={health.coverage.totalConstraints}
          />
        </div>

        {/* Issues */}
        {(['error', 'warning', 'info'] as Severity[]).map((severity) => {
          const items = grouped[severity];
          if (items.length === 0) return null;
          const config = SEVERITY_CONFIG[severity];
          return (
            <div key={severity} style={{ marginBottom: 16 }}>
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  textTransform: 'uppercase',
                  letterSpacing: 0.5,
                  color: config.color,
                  marginBottom: 6,
                }}
              >
                {config.label} ({items.length})
              </div>
              {items.map((issue, i) => (
                <IssueItem key={i} issue={issue} config={config} onIssueClick={onIssueClick} />
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function CoverageBar({ label, value, total }: { label: string; value: number; total: number }) {
  const pct = total > 0 ? Math.round((value / total) * 100) : 100;
  const color = pct > 80 ? '#16a34a' : pct > 50 ? '#d97706' : '#dc2626';

  return (
    <div style={{ marginBottom: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: '#666', marginBottom: 2 }}>
        <span>{label}</span>
        <span>
          {value}/{total} ({pct}%)
        </span>
      </div>
      <div style={{ height: 4, background: '#f3f4f6', borderRadius: 2 }}>
        <div
          style={{
            height: '100%',
            width: `${pct}%`,
            background: color,
            borderRadius: 2,
            transition: 'width 0.3s',
          }}
        />
      </div>
    </div>
  );
}

function IssueItem({
  issue,
  config,
  onIssueClick,
}: {
  issue: HealthIssue;
  config: { color: string; bg: string };
  onIssueClick?: (elementType: string, elementId: string) => void;
}) {
  const clickable = issue.elementType && issue.elementId;

  return (
    <div
      onClick={() => {
        if (clickable) onIssueClick?.(issue.elementType!, issue.elementId!);
      }}
      style={{
        fontSize: 11,
        color: '#555',
        padding: '6px 8px',
        background: config.bg,
        borderRadius: 4,
        marginBottom: 4,
        cursor: clickable ? 'pointer' : 'default',
        borderLeft: `2px solid ${config.color}`,
      }}
    >
      {issue.message}
    </div>
  );
}
