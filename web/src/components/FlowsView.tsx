import { useState, useEffect, useMemo } from 'react';
import {
  ReactFlow,
  Controls,
  Background,
  BackgroundVariant,
  MiniMap,
  useNodesState,
  useEdgesState,
  type Node,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import SpecNode from './nodes/SpecNode';
import type { FdmlDocument } from '../api/types';
import type { InferredFlow } from '../layout/inferFlows';
import { flowsToGraph, getFlowList } from '../layout/flowsToGraph';
import { NODE_COLORS } from '../theme/colors';
import ELK, { type ElkNode, type ElkExtendedEdge } from 'elkjs/lib/elk.bundled.js';

const elk = new ELK();
const nodeTypes = { specNode: SpecNode };

interface FlowsViewProps {
  spec: FdmlDocument;
  inferredFlows: InferredFlow[];
}

async function layoutFlowGraph(nodes: Node[], edges: Array<{ id: string; source: string; target: string }>) {
  if (nodes.length === 0) return nodes;

  const elkNodes: ElkNode[] = nodes.map((n) => ({
    id: n.id,
    width: 220,
    height: 80,
  }));

  const elkEdges: ElkExtendedEdge[] = edges.map((e) => ({
    id: e.id,
    sources: [e.source],
    targets: [e.target],
  }));

  const graph: ElkNode = {
    id: 'root',
    children: elkNodes,
    edges: elkEdges,
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': 'RIGHT',
      'elk.spacing.nodeNode': '50',
      'elk.layered.spacing.nodeNodeBetweenLayers': '100',
      'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
    },
  };

  const layouted = await elk.layout(graph);

  return nodes.map((node) => {
    const elkNode = layouted.children?.find((n) => n.id === node.id);
    return {
      ...node,
      position: { x: elkNode?.x ?? 0, y: elkNode?.y ?? 0 },
    };
  });
}

export default function FlowsView({ spec, inferredFlows }: FlowsViewProps) {
  const flowList = useMemo(() => getFlowList(spec, inferredFlows), [spec, inferredFlows]);
  const [selectedFlow, setSelectedFlow] = useState<string | null>(null);

  const { nodes: rawNodes, edges: rawEdges } = useMemo(
    () => flowsToGraph(spec, inferredFlows, selectedFlow),
    [spec, inferredFlows, selectedFlow],
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(rawNodes);
  const [edges, , onEdgesChange] = useEdgesState(rawEdges);

  useEffect(() => {
    layoutFlowGraph(rawNodes, rawEdges).then((positioned) => {
      setNodes(positioned);
    });
  }, [rawNodes, rawEdges, setNodes]);

  const minimapColor = (node: Node) => {
    const data = node.data as { isRoot?: boolean };
    return data.isRoot ? NODE_COLORS.action.border : NODE_COLORS.action.bg;
  };

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
      {/* Flow selector */}
      <div
        style={{
          padding: '8px 16px',
          borderBottom: '1px solid #e5e7eb',
          background: '#fff',
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          fontSize: 12,
        }}
      >
        <span style={{ fontWeight: 600, color: '#666' }}>Flow:</span>
        <select
          value={selectedFlow || ''}
          onChange={(e) => setSelectedFlow(e.target.value || null)}
          style={{
            fontSize: 12,
            padding: '4px 8px',
            borderRadius: 4,
            border: '1px solid #d1d5db',
            background: '#fff',
          }}
        >
          <option value="">All flows</option>
          {flowList.map((f) => (
            <option key={f.id} value={f.id}>
              {f.name} ({f.type})
            </option>
          ))}
        </select>
        <span style={{ color: '#999', fontSize: 11 }}>
          {nodes.length} actions, {edges.length} connections
        </span>
      </div>

      {/* Graph area */}
      <div style={{ flex: 1 }}>
        {nodes.length === 0 ? (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: '#888' }}>
            No flows to display
          </div>
        ) : (
          <ReactFlow
            nodes={nodes}
            edges={rawEdges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            nodeTypes={nodeTypes}
            fitView
            fitViewOptions={{ padding: 0.3 }}
            minZoom={0.1}
            maxZoom={2}
          >
            <Controls />
            <MiniMap nodeColor={minimapColor} zoomable pannable />
            <Background variant={BackgroundVariant.Dots} gap={16} size={1} color="#e5e7eb" />
          </ReactFlow>
        )}
      </div>
    </div>
  );
}
