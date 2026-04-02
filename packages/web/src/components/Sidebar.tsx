import { useStore } from '../stores/useStore';
import {
  Users, MapPin, Shield, Eye, KeyRound, FolderOpen,
  ChevronDown, ChevronRight,
} from 'lucide-react';
import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import type { GraphNode } from '../lib/types';

const ENTITY_ICONS = {
  character: Users,
  location: MapPin,
  faction: Shield,
} as const;

const ENTITY_COLORS = {
  character: 'text-indigo-400',
  location: 'text-green-400',
  faction: 'text-amber-400',
};

const ENTITY_LABELS: Record<string, string> = {
  character: '角色',
  location: '地点',
  faction: '势力',
};

function EntityGroup({ type, nodes }: { type: string; nodes: GraphNode[] }) {
  const [open, setOpen] = useState(true);
  const { selectedNode, selectNode } = useStore();
  const Icon = ENTITY_ICONS[type as keyof typeof ENTITY_ICONS] || Users;
  const color = ENTITY_COLORS[type as keyof typeof ENTITY_COLORS] || 'text-zinc-400';

  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-xs font-medium text-zinc-500 uppercase tracking-wider hover:text-zinc-300 transition-colors"
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <Icon size={13} className={color} />
        <span>{ENTITY_LABELS[type] || type}</span>
        <span className="ml-auto text-zinc-600 font-normal">{nodes.length}</span>
      </button>
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            {nodes.map((node) => (
              <button
                key={node.id}
                onClick={() => selectNode(selectedNode?.id === node.id ? null : node)}
                className={`
                  w-full text-left px-3 py-1.5 pl-8 text-sm transition-colors
                  ${selectedNode?.id === node.id
                    ? 'bg-indigo-500/10 text-indigo-300 border-r-2 border-indigo-500'
                    : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'
                  }
                `}
              >
                {node.name}
              </button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export default function Sidebar() {
  const {
    projects, currentProject, selectProject,
    graph, phases, currentPhase, selectPhase,
    secretMode, setSecretMode,
  } = useStore();

  const phaseIds = phases
    ? (phases.plan.length > 0 ? phases.plan : Object.keys(phases.phases).sort())
    : [];

  const grouped: Record<string, GraphNode[]> = {};
  for (const node of graph?.nodes || []) {
    (grouped[node.type] ||= []).push(node);
  }

  return (
    <div className="w-56 flex-shrink-0 bg-[var(--bg-surface)] border-r border-[var(--border)] flex flex-col h-full">
      {/* Header */}
      <div className="px-4 py-3 border-b border-[var(--border)]">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-indigo-500 animate-pulse" />
          <span className="text-sm font-semibold tracking-wide text-zinc-200">Elore</span>
        </div>
      </div>

      {/* Project selector */}
      <div className="px-3 py-2 border-b border-[var(--border)]">
        <div className="flex items-center gap-1.5 mb-1">
          <FolderOpen size={11} className="text-zinc-500" />
          <label className="text-[10px] uppercase tracking-wider text-zinc-600 font-medium">Project</label>
        </div>
        <select
          value={currentProject || ''}
          onChange={(e) => selectProject(e.target.value)}
          className="w-full bg-[var(--bg-elevated)] text-zinc-300 text-xs border border-[var(--border)] rounded-md px-2 py-1.5 outline-none focus:border-indigo-500/50 transition-colors"
        >
          {projects.map((p) => (
            <option key={p.id} value={p.id}>{p.id}</option>
          ))}
        </select>
      </div>

      {/* Phase selector */}
      <div className="px-3 py-2 border-b border-[var(--border)]">
        <label className="text-[10px] uppercase tracking-wider text-zinc-600 font-medium">Phase</label>
        <select
          value={currentPhase || ''}
          onChange={(e) => selectPhase(e.target.value)}
          className="mt-1 w-full bg-[var(--bg-elevated)] text-zinc-300 text-xs border border-[var(--border)] rounded-md px-2 py-1.5 outline-none focus:border-indigo-500/50 transition-colors"
        >
          {phaseIds.map((id) => (
            <option key={id} value={id}>{id}</option>
          ))}
        </select>
        {currentPhase && phases?.phases[currentPhase] && (
          <div className="flex gap-3 mt-2 text-[10px] text-zinc-600">
            <span>{phases.phases[currentPhase].beats} beats</span>
            <span>{phases.phases[currentPhase].words} words</span>
          </div>
        )}
      </div>

      {/* Entity tree */}
      <div className="flex-1 overflow-y-auto py-2">
        {['character', 'location', 'faction'].map((type) =>
          grouped[type]?.length ? (
            <EntityGroup key={type} type={type} nodes={grouped[type]} />
          ) : null
        )}
      </div>

      {/* Secret mode */}
      <div className="px-3 py-2 border-t border-[var(--border)]">
        <div className="flex items-center gap-1.5 mb-1.5">
          <KeyRound size={11} className="text-purple-400" />
          <span className="text-[10px] uppercase tracking-wider text-zinc-600 font-medium">Secrets</span>
        </div>
        <div className="flex rounded-md overflow-hidden border border-[var(--border)]">
          <button
            onClick={() => setSecretMode('reader')}
            className={`flex-1 text-[11px] py-1 flex items-center justify-center gap-1 transition-colors ${
              secretMode === 'reader'
                ? 'bg-indigo-500/15 text-indigo-300'
                : 'bg-[var(--bg-elevated)] text-zinc-500 hover:text-zinc-300'
            }`}
          >
            <Eye size={11} /> 读者
          </button>
          <button
            onClick={() => setSecretMode('omniscient')}
            className={`flex-1 text-[11px] py-1 flex items-center justify-center gap-1 transition-colors border-l border-[var(--border)] ${
              secretMode === 'omniscient'
                ? 'bg-purple-500/15 text-purple-300'
                : 'bg-[var(--bg-elevated)] text-zinc-500 hover:text-zinc-300'
            }`}
          >
            <Eye size={11} /> 全知
          </button>
        </div>
      </div>
    </div>
  );
}
