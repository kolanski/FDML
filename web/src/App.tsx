import { useState, useMemo, useCallback, useEffect } from 'react';
import type { FdmlDocument } from './api/types';
import { useSpec } from './hooks/useSpec';
import { useLayout } from './hooks/useLayout';
import { buildHierarchy } from './layout/buildHierarchy';
import { analyzeHealth } from './layout/specHealth';
import HealthPanel from './components/HealthPanel';
import GenerationView from './components/GenerationView';
import Toolbar from './components/toolbar/Toolbar';
import { VIEW_BLOCKS, type ViewContext } from './views';

export type ViewMode = string;

export default function App() {
  const { spec, error, loading, refetch } = useSpec();
  const [activeView, setActiveView] = useState<ViewMode>('spec');
  const [showHealth, setShowHealth] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [drillSpec, setDrillSpec] = useState<FdmlDocument | null>(null);
  const [drillSystemName, setDrillSystemName] = useState<string | null>(null);

  // Check if generation is in progress on mount
  useEffect(() => {
    fetch('/api/generation-status')
      .then((r) => r.json())
      .then((s) => {
        if (s.phase === 'scanning' || s.phase === 'generating') {
          setActiveView('generation');
        }
      })
      .catch(() => {});
  }, []);

  // ALL hooks must be before any early return
  const { nodes, edges } = useLayout(spec);

  const hierarchy = useMemo(() => (spec ? buildHierarchy(spec) : null), [spec]);
  const health = useMemo(
    () => (spec && hierarchy ? analyzeHealth(spec, hierarchy) : null),
    [spec, hierarchy],
  );

  const drillHierarchy = useMemo(
    () => (drillSpec ? buildHierarchy(drillSpec) : null),
    [drillSpec],
  );
  const drillHealth = useMemo(
    () => (drillSpec && drillHierarchy ? analyzeHealth(drillSpec, drillHierarchy) : null),
    [drillSpec, drillHierarchy],
  );

  const handleIssueClick = useCallback((elementType: string, elementId: string) => {
    setActiveView('spec');
    setShowHealth(false);
    setTimeout(() => {
      const el = document.getElementById(`${elementType}-${elementId}`);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'center' });
        el.style.boxShadow = '0 0 0 2px #3b82f6';
        setTimeout(() => {
          el.style.boxShadow = '0 1px 3px rgba(0,0,0,0.06)';
        }, 1500);
      }
    }, 100);
  }, []);

  const handleDrillDown = useCallback(async (systemId: string) => {
    try {
      const res = await fetch(`/api/spec/${systemId}`);
      if (!res.ok) return;
      const subDoc: FdmlDocument = await res.json();
      if (subDoc.entities.length > 0 || subDoc.actions.length > 0) {
        const sysName = spec?.systems.find((s) => s.id === systemId)?.name || systemId;
        setDrillSpec(subDoc);
        setDrillSystemName(sysName);
        setActiveView('spec');
      }
    } catch { /* ignore */ }
  }, [spec]);

  const handleBackToPlatform = useCallback(() => {
    setDrillSpec(null);
    setDrillSystemName(null);
    setActiveView('architecture');
  }, []);

  const handleGenerationComplete = useCallback(() => {
    refetch();
    setActiveView('spec');
  }, [refetch]);

  const handleViewChange = useCallback((view: ViewMode) => {
    if (view === 'architecture' && drillSpec) {
      setDrillSpec(null);
      setDrillSystemName(null);
    }
    setActiveView(view);
  }, [drillSpec]);

  // --- Early returns (after all hooks) ---

  if (activeView === 'generation') {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
        <div style={{
          height: 44, background: '#fff', borderBottom: '1px solid #e5e7eb',
          display: 'flex', alignItems: 'center', padding: '0 16px', gap: 12, fontSize: 13, flexShrink: 0,
        }}>
          <span style={{ fontWeight: 700, fontSize: 14 }}>FDML — Generating...</span>
        </div>
        <div style={{ flex: 1, overflow: 'hidden' }}>
          <GenerationView onComplete={handleGenerationComplete} />
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', color: '#888' }}>
        Loading spec...
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', color: '#dc2626' }}>
        Error: {error}
      </div>
    );
  }

  if (!spec || !hierarchy) return null;

  // --- Derived state (after null checks) ---

  const activeSpec = drillSpec || spec;
  const activeHierarchy = drillHierarchy || hierarchy;
  const activeHealth = drillHealth || health;

  const counts: Record<string, number> = {
    entity: activeSpec.entities.length,
    action: activeSpec.actions.length,
    feature: activeSpec.features.length,
    constraint: activeSpec.constraints.length,
    flow: activeSpec.flows.length + (activeHierarchy?.inferredFlows.length || 0),
    system: spec.systems.length,
    integration: spec.integrations.length,
  };

  // The whole contract between App and the view blocks. Blocks read only from this.
  const ctx: ViewContext = {
    spec: activeSpec,
    platformSpec: spec,
    hierarchy: activeHierarchy,
    searchQuery,
    nodes,
    edges,
    onDrillDown: handleDrillDown,
  };
  const tabs = VIEW_BLOCKS.filter((b) => !b.available || b.available(ctx)).map((b) => ({
    id: b.id,
    label: b.label,
  }));

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      <Toolbar
        systemName={drillSystemName || spec.system?.name}
        counts={counts}
        activeView={activeView}
        onViewChange={handleViewChange}
        healthScore={activeHealth?.score ?? null}
        onHealthClick={() => setShowHealth(!showHealth)}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        tabs={tabs}
      />
      {drillSpec && (
        <div style={{
          background: '#eff6ff', borderBottom: '1px solid #bfdbfe',
          padding: '4px 16px', fontSize: 12, display: 'flex', alignItems: 'center', gap: 8,
        }}>
          <button
            onClick={handleBackToPlatform}
            style={{
              background: '#3b82f6', color: '#fff', border: 'none', borderRadius: 4,
              padding: '2px 10px', fontSize: 11, cursor: 'pointer', fontWeight: 600,
            }}
          >
            &larr; Back to Platform
          </button>
          <span style={{ color: '#1e40af' }}>Viewing: {drillSystemName}</span>
        </div>
      )}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          {(VIEW_BLOCKS.find((b) => b.id === activeView) ?? VIEW_BLOCKS[0]).render(ctx)}
        </div>
        {showHealth && activeHealth && (
          <HealthPanel
            health={activeHealth}
            onClose={() => setShowHealth(false)}
            onIssueClick={handleIssueClick}
          />
        )}
      </div>
    </div>
  );
}
