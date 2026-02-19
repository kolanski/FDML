import { useState, useMemo, useCallback } from 'react';
import { useSpec } from './hooks/useSpec';
import { useLayout } from './hooks/useLayout';
import { buildHierarchy } from './layout/buildHierarchy';
import { analyzeHealth } from './layout/specHealth';
import SpecView from './components/SpecView';
import Canvas from './components/Canvas';
import FlowsView from './components/FlowsView';
import HealthPanel from './components/HealthPanel';
import Toolbar from './components/toolbar/Toolbar';

export type ViewMode = 'spec' | 'graph' | 'flows';

export default function App() {
  const { spec, error, loading } = useSpec();
  const [activeView, setActiveView] = useState<ViewMode>('spec');
  const [showHealth, setShowHealth] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');

  // Always call hooks unconditionally
  const { nodes, edges } = useLayout(spec);

  const hierarchy = useMemo(() => (spec ? buildHierarchy(spec) : null), [spec]);
  const health = useMemo(
    () => (spec && hierarchy ? analyzeHealth(spec, hierarchy) : null),
    [spec, hierarchy],
  );

  const handleIssueClick = useCallback((elementType: string, elementId: string) => {
    // Switch to spec view and scroll to element
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

  const counts: Record<string, number> = {
    entity: spec.entities.length,
    action: spec.actions.length,
    feature: spec.features.length,
    constraint: spec.constraints.length,
    flow: spec.flows.length + hierarchy.inferredFlows.length,
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      <Toolbar
        systemName={spec.system?.name}
        counts={counts}
        activeView={activeView}
        onViewChange={setActiveView}
        healthScore={health?.score ?? null}
        onHealthClick={() => setShowHealth(!showHealth)}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
      />
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          {activeView === 'spec' && (
            <SpecView spec={spec} searchQuery={searchQuery} hierarchy={hierarchy} />
          )}
          {activeView === 'graph' && (
            <Canvas initialNodes={nodes} initialEdges={edges} />
          )}
          {activeView === 'flows' && (
            <FlowsView spec={spec} inferredFlows={hierarchy.inferredFlows} />
          )}
        </div>
        {showHealth && health && (
          <HealthPanel
            health={health}
            onClose={() => setShowHealth(false)}
            onIssueClick={handleIssueClick}
          />
        )}
      </div>
    </div>
  );
}
