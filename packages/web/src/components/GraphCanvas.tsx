import { useCallback, useEffect, useMemo } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  BackgroundVariant,
  type Node,
  type Edge,
  MarkerType,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import { useStore } from '../stores/useStore';
import EntityNode from './EntityNode';
import type { GraphNode } from '../lib/types';

const nodeTypes = { entity: EntityNode };

const EDGE_COLORS: Record<string, string> = {
  relationship: '#6366f1',
  location: '#22c55e',
  membership: '#f59e0b',
  rivalry: '#ef4444',
  connection: '#3f3f46',
  goal_conflict: '#ef4444',
};

const EDGE_STYLES: Record<string, string> = {
  connection: '6 4',
  goal_conflict: '4 4',
};

// Simple force-directed layout
function computeLayout(nodes: GraphNode[], edges: { source: string; target: string }[]): Map<string, { x: number; y: number }> {
  const positions = new Map<string, { x: number; y: number }>();

  // Group by type for initial placement
  const groups: Record<string, GraphNode[]> = {};
  for (const n of nodes) {
    (groups[n.type] ||= []).push(n);
  }

  const typeOrder = ['character', 'faction', 'location'];
  let globalIdx = 0;
  for (const type of typeOrder) {
    const group = groups[type] || [];
    const cols = Math.ceil(Math.sqrt(group.length));
    group.forEach((node, i) => {
      const row = Math.floor(i / cols);
      const col = i % cols;
      const baseX = typeOrder.indexOf(type) * 400;
      positions.set(node.id, {
        x: baseX + col * 220 + (Math.random() - 0.5) * 40,
        y: row * 140 + (Math.random() - 0.5) * 30,
      });
      globalIdx++;
    });
  }

  // Simple force simulation (few iterations)
  const adjacency = new Map<string, Set<string>>();
  for (const e of edges) {
    if (!adjacency.has(e.source)) adjacency.set(e.source, new Set());
    if (!adjacency.has(e.target)) adjacency.set(e.target, new Set());
    adjacency.get(e.source)!.add(e.target);
    adjacency.get(e.target)!.add(e.source);
  }

  for (let iter = 0; iter < 80; iter++) {
    const forces = new Map<string, { fx: number; fy: number }>();
    for (const n of nodes) forces.set(n.id, { fx: 0, fy: 0 });

    // Repulsion
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const a = positions.get(nodes[i].id)!;
        const b = positions.get(nodes[j].id)!;
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const force = 8000 / (dist * dist);
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        forces.get(nodes[i].id)!.fx += fx;
        forces.get(nodes[i].id)!.fy += fy;
        forces.get(nodes[j].id)!.fx -= fx;
        forces.get(nodes[j].id)!.fy -= fy;
      }
    }

    // Attraction (edges)
    for (const e of edges) {
      const a = positions.get(e.source);
      const b = positions.get(e.target);
      if (!a || !b) continue;
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = (dist - 180) * 0.01;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;
      forces.get(e.source)!.fx += fx;
      forces.get(e.source)!.fy += fy;
      forces.get(e.target)!.fx -= fx;
      forces.get(e.target)!.fy -= fy;
    }

    // Apply
    const damping = 0.85 - iter * 0.005;
    for (const n of nodes) {
      const pos = positions.get(n.id)!;
      const f = forces.get(n.id)!;
      pos.x += f.fx * damping;
      pos.y += f.fy * damping;
    }
  }

  return positions;
}

export default function GraphCanvas() {
  const { graph, selectedNode, selectNode, activeEdgeTypes } = useStore();
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

  // Convert graph data to React Flow format
  useEffect(() => {
    if (!graph) return;

    const positions = computeLayout(graph.nodes, graph.edges);

    const rfNodes: Node[] = graph.nodes.map((n) => ({
      id: n.id,
      type: 'entity',
      position: positions.get(n.id) || { x: 0, y: 0 },
      data: {
        ...n,
        selected: selectedNode?.id === n.id,
        hasSecrets: graph.secrets.some((s) => s.known_by.includes(n.id)),
      },
    }));

    const rfEdges: Edge[] = graph.edges.map((e, i) => ({
      id: `e-${i}`,
      source: e.source,
      target: e.target,
      type: 'default',
      animated: e.type === 'relationship',
      hidden: !activeEdgeTypes.has(e.type),
      label: e.label || undefined,
      labelStyle: { fill: '#71717a', fontSize: 10 },
      labelBgStyle: { fill: '#09090b', fillOpacity: 0.8 },
      labelBgPadding: [4, 2] as [number, number],
      labelBgBorderRadius: 4,
      style: {
        stroke: EDGE_COLORS[e.type] || '#3f3f46',
        strokeWidth: e.type === 'relationship' ? 1.5 : 1,
        strokeDasharray: EDGE_STYLES[e.type],
        opacity: 0.5,
      },
      markerEnd: e.type === 'relationship' ? {
        type: MarkerType.ArrowClosed,
        color: EDGE_COLORS[e.type],
        width: 16,
        height: 16,
      } : undefined,
    }));

    setNodes(rfNodes);
    setEdges(rfEdges);
  }, [graph, activeEdgeTypes]);

  // Update selected state on nodes when selection changes
  useEffect(() => {
    setNodes((nds) =>
      nds.map((n) => ({
        ...n,
        data: { ...n.data, selected: selectedNode?.id === n.id },
      }))
    );
  }, [selectedNode]);

  const onNodeClick = useCallback(
    (_: any, node: Node) => {
      const graphNode = graph?.nodes.find((n) => n.id === node.id);
      if (graphNode) {
        selectNode(selectedNode?.id === graphNode.id ? null : graphNode);
      }
    },
    [graph, selectedNode, selectNode]
  );

  const onPaneClick = useCallback(() => {
    selectNode(null);
  }, [selectNode]);

  // Edge filter legend
  const edgeTypes = [
    { key: 'relationship', label: '关系', color: '#6366f1' },
    { key: 'location', label: '位置', color: '#22c55e' },
    { key: 'membership', label: '隶属', color: '#f59e0b' },
    { key: 'rivalry', label: '对立', color: '#ef4444' },
    { key: 'connection', label: '连接', color: '#3f3f46' },
    { key: 'goal_conflict', label: '冲突', color: '#ef4444' },
  ];

  const { toggleEdgeType } = useStore();

  return (
    <div className="flex-1 relative">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={onNodeClick}
        onPaneClick={onPaneClick}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.3 }}
        minZoom={0.2}
        maxZoom={2}
        proOptions={{ hideAttribution: true }}
      >
        <Background variant={BackgroundVariant.Dots} gap={24} size={1} color="#1a1a22" />
        <Controls showInteractive={false} />
        <MiniMap
          nodeColor={(n) => {
            const type = (n.data as any)?.type;
            if (type === 'character') return '#6366f1';
            if (type === 'location') return '#22c55e';
            if (type === 'faction') return '#f59e0b';
            return '#3f3f46';
          }}
          maskColor="rgba(0,0,0,0.7)"
          style={{ background: '#0f0f13' }}
        />
      </ReactFlow>

      {/* Edge filter overlay */}
      <div className="absolute top-3 right-3 flex gap-1.5 bg-[var(--bg-surface)]/90 backdrop-blur-sm border border-[var(--border)] rounded-lg px-2 py-1.5">
        {edgeTypes.map(({ key, label, color }) => (
          <button
            key={key}
            onClick={() => toggleEdgeType(key)}
            className={`
              flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium
              transition-all duration-150
              ${activeEdgeTypes.has(key)
                ? 'text-zinc-200'
                : 'text-zinc-600 opacity-50'
              }
            `}
          >
            <div
              className="w-2 h-2 rounded-full transition-opacity"
              style={{
                backgroundColor: color,
                opacity: activeEdgeTypes.has(key) ? 1 : 0.3,
              }}
            />
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
