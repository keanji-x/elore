import { useStore } from '../stores/useStore';
import { Network, LayoutGrid, GitBranch } from 'lucide-react';

export default function Toolbar() {
  const { currentPhase, phases } = useStore();
  const phaseInfo = currentPhase && phases?.phases[currentPhase];
  const status = phaseInfo?.status;

  const statusColors: Record<string, string> = {
    locked: 'bg-zinc-600',
    ready: 'bg-blue-500',
    active: 'bg-green-500',
    reviewing: 'bg-amber-500',
    approved: 'bg-indigo-500',
  };

  return (
    <div className="h-10 flex-shrink-0 bg-[var(--bg-surface)] border-b border-[var(--border)] flex items-center px-4 gap-4">
      {/* View switcher */}
      <div className="flex items-center gap-0.5 bg-[var(--bg-elevated)] rounded-md p-0.5 border border-[var(--border)]">
        <button className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium bg-indigo-500/15 text-indigo-300">
          <Network size={12} />
          Graph
        </button>
        <button className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium text-zinc-500 hover:text-zinc-300 transition-colors" title="Coming soon">
          <LayoutGrid size={12} />
          Timeline
        </button>
        <button className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium text-zinc-500 hover:text-zinc-300 transition-colors" title="Coming soon">
          <GitBranch size={12} />
          Board
        </button>
      </div>

      {/* Phase info */}
      {phaseInfo && (
        <div className="flex items-center gap-2 ml-auto text-xs text-zinc-500">
          <div className={`w-1.5 h-1.5 rounded-full ${statusColors[status || ''] || 'bg-zinc-600'}`} />
          <span className="text-zinc-400">{currentPhase}</span>
          <span className="text-zinc-600">·</span>
          <span>{phaseInfo.beats} beats</span>
          <span className="text-zinc-600">·</span>
          <span>{phaseInfo.words} 字</span>
          <span className="text-zinc-600">·</span>
          <span>{phaseInfo.effects} effects</span>
        </div>
      )}
    </div>
  );
}
