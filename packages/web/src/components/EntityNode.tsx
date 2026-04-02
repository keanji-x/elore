import { memo } from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';
import { User, MapPin, Shield, KeyRound } from 'lucide-react';
import { motion } from 'framer-motion';
import type { GraphNode } from '../lib/types';

const TYPE_CONFIG = {
  character: {
    icon: User,
    bg: 'bg-indigo-500/10',
    border: 'border-indigo-500/30',
    borderSelected: 'border-indigo-400',
    dot: 'bg-indigo-400',
    glow: 'shadow-indigo-500/20',
    ring: 'ring-indigo-500/30',
  },
  location: {
    icon: MapPin,
    bg: 'bg-green-500/10',
    border: 'border-green-500/30',
    borderSelected: 'border-green-400',
    dot: 'bg-green-400',
    glow: 'shadow-green-500/20',
    ring: 'ring-green-500/30',
  },
  faction: {
    icon: Shield,
    bg: 'bg-amber-500/10',
    border: 'border-amber-500/30',
    borderSelected: 'border-amber-400',
    dot: 'bg-amber-400',
    glow: 'shadow-amber-500/20',
    ring: 'ring-amber-500/30',
  },
};

type EntityNodeData = GraphNode & { selected?: boolean; hasSecrets?: boolean };

function EntityNodeComponent({ data }: NodeProps<any>) {
  const d = data as EntityNodeData;
  const cfg = TYPE_CONFIG[d.type] || TYPE_CONFIG.character;
  const Icon = cfg.icon;
  const isSelected = d.selected;

  return (
    <motion.div
      initial={{ scale: 0, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      transition={{ type: 'spring', stiffness: 300, damping: 25 }}
      className="relative"
    >
      <Handle type="target" position={Position.Top} className="!bg-transparent !border-0 !w-0 !h-0" />
      <Handle type="source" position={Position.Bottom} className="!bg-transparent !border-0 !w-0 !h-0" />

      <div
        className={`
          relative px-3 py-2 rounded-lg border backdrop-blur-sm
          transition-all duration-200 cursor-pointer
          ${cfg.bg} ${isSelected ? cfg.borderSelected : cfg.border}
          ${isSelected ? `shadow-lg ${cfg.glow} ring-1 ${cfg.ring}` : 'hover:shadow-md'}
        `}
        style={{ minWidth: 80 }}
      >
        <div className="flex items-center gap-2">
          <div className={`w-1.5 h-1.5 rounded-full ${cfg.dot}`} />
          <Icon size={13} className="text-zinc-400" />
          <span className="text-xs font-medium text-zinc-200 whitespace-nowrap">
            {d.name}
          </span>
          {d.hasSecrets && (
            <KeyRound size={10} className="text-purple-400 ml-0.5" />
          )}
        </div>

        {/* Subtle sub-info */}
        {d.type === 'character' && d.traits?.length > 0 && (
          <div className="mt-1 flex gap-1 flex-wrap">
            {d.traits.slice(0, 3).map((t) => (
              <span key={t} className="text-[9px] px-1.5 py-0 rounded bg-zinc-800/60 text-zinc-500">
                {t}
              </span>
            ))}
          </div>
        )}
      </div>
    </motion.div>
  );
}

export default memo(EntityNodeComponent);
