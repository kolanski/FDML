import { useCallback, useMemo } from 'react';
import {
  ReactFlow,
  MiniMap,
  Controls,
  Background,
  BackgroundVariant,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type NodeMouseHandler,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import SpecNode from './nodes/SpecNode';
import type { SpecNodeData } from '../layout/specToGraph';
import { NODE_COLORS } from '../theme/colors';
import { layoutGraph } from '../layout/elkLayout';

interface CanvasProps {
  initialNodes: Node[];
  initialEdges: Edge[];
}

const nodeTypes = { specNode: SpecNode };

export default function Canvas({ initialNodes, initialEdges }: CanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, , onEdgesChange] = useEdgesState(initialEdges);

  // Sync when initialNodes change (SSE reload)
  useMemo(() => {
    setNodes(initialNodes);
  }, [initialNodes, setNodes]);

  const onNodeClick: NodeMouseHandler = useCallback(
    (_, node) => {
      setNodes((prev) => {
        const updated = prev.map((n) => {
          if (n.id !== node.id) return n;
          return {
            ...n,
            data: { ...n.data, expanded: !(n.data as unknown as SpecNodeData).expanded },
          };
        });
        // Re-layout after toggle
        layoutGraph(updated, edges).then((positioned) => setNodes(positioned));
        return updated;
      });
    },
    [edges, setNodes],
  );

  const minimapNodeColor = useCallback((node: Node) => {
    const d = node.data as unknown as SpecNodeData;
    return NODE_COLORS[d.nodeType]?.border || '#999';
  }, []);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onNodeClick={onNodeClick}
      nodeTypes={nodeTypes}
      fitView
      fitViewOptions={{ padding: 0.2 }}
      minZoom={0.1}
      maxZoom={2}
    >
      <Controls />
      <MiniMap nodeColor={minimapNodeColor} zoomable pannable />
      <Background variant={BackgroundVariant.Dots} gap={16} size={1} color="#e5e7eb" />
    </ReactFlow>
  );
}
