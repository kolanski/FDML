import { useState, useEffect, useCallback } from 'react';
import type { Node, Edge } from '@xyflow/react';
import type { FdmlDocument } from '../api/types';
import { specToGraph } from '../layout/specToGraph';
import { layoutGraph } from '../layout/elkLayout';

export function useLayout(spec: FdmlDocument | null) {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [layoutReady, setLayoutReady] = useState(false);

  useEffect(() => {
    if (!spec) return;
    const { nodes: rawNodes, edges: rawEdges } = specToGraph(spec);
    setEdges(rawEdges);
    layoutGraph(rawNodes, rawEdges).then((positioned) => {
      setNodes(positioned);
      setLayoutReady(true);
    });
  }, [spec]);

  const toggleExpand = useCallback((nodeId: string) => {
    setNodes((prev) => {
      const updated = prev.map((n) => {
        if (n.id !== nodeId) return n;
        return {
          ...n,
          data: { ...n.data, expanded: !n.data.expanded },
        };
      });
      // Re-layout after toggle
      const currentEdges = edges;
      layoutGraph(updated, currentEdges).then(setNodes);
      return updated;
    });
  }, [edges]);

  return { nodes, edges, setNodes, setEdges, layoutReady, toggleExpand };
}
