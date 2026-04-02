import { useStore } from '../stores/useStore';
import { motion, AnimatePresence } from 'framer-motion';
import {
  X, User, MapPin, Shield, Target, Handshake, KeyRound,
  ArrowRight, Swords, Package, Tag, FileText, Link, Crown,
} from 'lucide-react';

function Badge({ children, color = 'zinc' }: { children: React.ReactNode; color?: string }) {
  const colorMap: Record<string, string> = {
    zinc: 'bg-zinc-800 text-zinc-300 border-zinc-700',
    indigo: 'bg-indigo-500/10 text-indigo-300 border-indigo-500/20',
    green: 'bg-green-500/10 text-green-300 border-green-500/20',
    amber: 'bg-amber-500/10 text-amber-300 border-amber-500/20',
    purple: 'bg-purple-500/10 text-purple-300 border-purple-500/20',
    red: 'bg-red-500/10 text-red-300 border-red-500/20',
  };
  return (
    <span className={`inline-flex items-center px-2 py-0.5 text-[11px] rounded-md border ${colorMap[color] || colorMap.zinc}`}>
      {children}
    </span>
  );
}

function Section({ icon: Icon, title, children }: {
  icon: React.ComponentType<{ size?: number; className?: string }>;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-4">
      <div className="flex items-center gap-1.5 mb-2">
        <Icon size={12} className="text-zinc-500" />
        <h3 className="text-[10px] uppercase tracking-wider text-zinc-500 font-medium">{title}</h3>
      </div>
      {children}
    </div>
  );
}

export default function Inspector() {
  const { selectedNode, selectNode, graph, secretMode } = useStore();

  if (!selectedNode) {
    return (
      <div className="w-72 flex-shrink-0 bg-[var(--bg-surface)] border-l border-[var(--border)] flex items-center justify-center">
        <div className="text-center text-zinc-600">
          <div className="text-3xl mb-2 opacity-30">⬡</div>
          <p className="text-xs">选择一个节点查看详情</p>
        </div>
      </div>
    );
  }

  const node = selectedNode;
  const typeConfig = {
    character: { icon: User, color: 'indigo', label: '角色', dot: 'bg-indigo-500' },
    location: { icon: MapPin, color: 'green', label: '地点', dot: 'bg-green-500' },
    faction: { icon: Shield, color: 'amber', label: '势力', dot: 'bg-amber-500' },
  };
  const cfg = typeConfig[node.type] || typeConfig.character;

  // Find relationships from this node
  const rels = graph?.edges.filter(
    (e) => e.type === 'relationship' && e.source === node.id
  ) || [];

  // Find secrets
  const secrets = graph?.secrets.filter((s) => {
    if (secretMode === 'reader' && !s.revealed_to_reader) return false;
    return s.known_by.includes(node.id);
  }) || [];

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={node.id}
        initial={{ opacity: 0, x: 20 }}
        animate={{ opacity: 1, x: 0 }}
        exit={{ opacity: 0, x: 20 }}
        transition={{ duration: 0.15 }}
        className="w-72 flex-shrink-0 bg-[var(--bg-surface)] border-l border-[var(--border)] flex flex-col h-full"
      >
        {/* Header */}
        <div className="px-4 py-3 border-b border-[var(--border)] flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${cfg.dot}`} />
            <div>
              <h2 className="text-sm font-medium text-zinc-100">{node.name}</h2>
              <span className="text-[10px] text-zinc-500 uppercase tracking-wider">{cfg.label}</span>
            </div>
          </div>
          <button
            onClick={() => selectNode(null)}
            className="p-1 rounded hover:bg-zinc-800 text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            <X size={14} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-4 py-3">
          {/* Description */}
          {node.description && (
            <Section icon={FileText} title="描述">
              <p className="text-xs text-zinc-400 leading-relaxed whitespace-pre-wrap">{node.description}</p>
            </Section>
          )}

          {/* Traits */}
          {node.traits?.length > 0 && (
            <Section icon={User} title="特征">
              <div className="flex flex-wrap gap-1.5">
                {node.traits.map((t) => <Badge key={t} color={cfg.color}>{t}</Badge>)}
              </div>
            </Section>
          )}

          {/* Properties (locations) */}
          {node.properties?.length > 0 && (
            <Section icon={MapPin} title="属性">
              <div className="flex flex-wrap gap-1.5">
                {node.properties.map((p) => <Badge key={p} color="green">{p}</Badge>)}
              </div>
            </Section>
          )}

          {/* Members (factions) */}
          {node.members?.length > 0 && (
            <Section icon={Shield} title="成员">
              <div className="flex flex-wrap gap-1.5">
                {node.members.map((m) => {
                  const mn = graph?.nodes.find((n) => n.id === m);
                  return <Badge key={m} color="amber">{mn?.name || m}</Badge>;
                })}
              </div>
            </Section>
          )}

          {/* Alignment */}
          {node.alignment && (
            <Section icon={Shield} title="立场">
              <p className="text-xs text-zinc-300">{node.alignment}</p>
            </Section>
          )}

          {/* Location */}
          {node.location && (
            <Section icon={MapPin} title="位置">
              <Badge color="green">
                {graph?.nodes.find((n) => n.id === node.location)?.name || node.location}
              </Badge>
            </Section>
          )}

          {/* Beliefs */}
          {node.beliefs?.length > 0 && (
            <Section icon={Target} title="信念">
              <ul className="space-y-1">
                {node.beliefs.map((b, i) => (
                  <li key={i} className="text-xs text-zinc-400 pl-2 border-l-2 border-zinc-700">{b}</li>
                ))}
              </ul>
            </Section>
          )}

          {/* Desires */}
          {node.desires?.length > 0 && (
            <Section icon={Target} title="欲望">
              <ul className="space-y-1">
                {node.desires.map((d, i) => (
                  <li key={i} className="text-xs text-zinc-400 pl-2 border-l-2 border-indigo-800">{d}</li>
                ))}
              </ul>
            </Section>
          )}

          {/* Goals */}
          {node.goals?.length > 0 && (
            <Section icon={Target} title="目标">
              <div className="space-y-2">
                {node.goals.map((g) => (
                  <div key={g.id} className="p-2 rounded-md bg-[var(--bg-elevated)] border border-[var(--border)]">
                    <div className="flex items-center gap-1.5 mb-1">
                      <Badge color={g.status === 'active' ? 'indigo' : 'zinc'}>{g.status}</Badge>
                      <span className="text-[10px] text-zinc-500">{g.id}</span>
                    </div>
                    <p className="text-xs text-zinc-300">{g.want}</p>
                    {g.conflicts_with?.length > 0 && (
                      <div className="flex items-center gap-1 mt-1.5">
                        <Swords size={10} className="text-red-400" />
                        <span className="text-[10px] text-red-400">
                          {g.conflicts_with.join(', ')}
                        </span>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </Section>
          )}

          {/* Inventory */}
          {node.inventory?.length > 0 && (
            <Section icon={Package} title="物品">
              <div className="flex flex-wrap gap-1.5">
                {node.inventory.map((item) => <Badge key={item}>{item}</Badge>)}
              </div>
            </Section>
          )}

          {/* Connections (locations) */}
          {node.connections?.length > 0 && (
            <Section icon={Link} title="连接">
              <div className="flex flex-wrap gap-1.5">
                {node.connections.map((c) => {
                  const cn = graph?.nodes.find((n) => n.id === c);
                  return <Badge key={c} color="green">{cn?.name || c}</Badge>;
                })}
              </div>
            </Section>
          )}

          {/* Rivals (factions) */}
          {node.rivals?.length > 0 && (
            <Section icon={Swords} title="对手">
              <div className="flex flex-wrap gap-1.5">
                {node.rivals.map((r) => {
                  const rn = graph?.nodes.find((n) => n.id === r);
                  return <Badge key={r} color="red">{rn?.name || r}</Badge>;
                })}
              </div>
            </Section>
          )}

          {/* Leader (factions) */}
          {node.leader && (
            <Section icon={Crown} title="领袖">
              <Badge color="amber">
                {graph?.nodes.find((n) => n.id === node.leader)?.name || node.leader}
              </Badge>
            </Section>
          )}

          {/* Tags */}
          {node.tags?.length > 0 && (
            <Section icon={Tag} title="标签">
              <div className="flex flex-wrap gap-1.5">
                {node.tags.map((t) => <Badge key={t}>{t}</Badge>)}
              </div>
            </Section>
          )}

          {/* Relationships */}
          {rels.length > 0 && (
            <Section icon={Handshake} title="关系">
              <div className="space-y-1.5">
                {rels.map((r, i) => {
                  const target = graph?.nodes.find((n) => n.id === r.target);
                  return (
                    <div key={i} className="flex items-center gap-2 p-1.5 rounded bg-[var(--bg-elevated)] text-xs">
                      <span className="text-zinc-500">{r.label}</span>
                      <ArrowRight size={10} className="text-zinc-600" />
                      <span className="text-zinc-300">{target?.name || r.target}</span>
                      {r.trust != null && (
                        <div className="ml-auto flex gap-1.5 text-[10px] text-zinc-600">
                          <span title="信任">T{r.trust}</span>
                          <span title="亲和">A{r.affinity}</span>
                          <span title="尊重">R{r.respect}</span>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </Section>
          )}

          {/* Secrets */}
          {secrets.length > 0 && (
            <Section icon={KeyRound} title="秘密">
              <div className="space-y-2">
                {secrets.map((s) => (
                  <div key={s.id} className="p-2 rounded-md bg-purple-500/5 border border-purple-500/15">
                    <div className="flex items-center gap-1.5 mb-1">
                      <span className="text-[10px] font-medium text-purple-400">{s.id}</span>
                      {s.dramatic_function && (
                        <Badge color="purple">{s.dramatic_function}</Badge>
                      )}
                    </div>
                    <p className="text-xs text-zinc-300">{s.content}</p>
                    <p className="text-[10px] text-zinc-600 mt-1">
                      知情者: {s.known_by.join(', ')}
                    </p>
                  </div>
                ))}
              </div>
            </Section>
          )}
        </div>
      </motion.div>
    </AnimatePresence>
  );
}
