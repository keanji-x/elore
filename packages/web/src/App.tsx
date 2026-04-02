import { useEffect } from 'react';
import { useStore } from './stores/useStore';
import Sidebar from './components/Sidebar';
import Toolbar from './components/Toolbar';
import GraphCanvas from './components/GraphCanvas';
import Inspector from './components/Inspector';
import { motion } from 'framer-motion';

export default function App() {
  const { init, loading, error, graph } = useStore();

  useEffect(() => {
    init();
  }, [init]);

  if (error) {
    return (
      <div className="h-screen flex items-center justify-center bg-[var(--bg-base)]">
        <div className="text-center max-w-md">
          <div className="text-4xl mb-4 opacity-20">⬡</div>
          <h1 className="text-lg font-medium text-zinc-200 mb-2">连接失败</h1>
          <p className="text-sm text-zinc-500 mb-4">
            无法连接到 Elore 服务器。请确认 <code className="text-indigo-400">elore serve</code> 正在运行。
          </p>
          <pre className="text-xs text-red-400/70 bg-red-500/5 border border-red-500/10 rounded-lg p-3 text-left overflow-auto">
            {error}
          </pre>
          <button
            onClick={() => init()}
            className="mt-4 px-4 py-2 text-sm bg-indigo-500/15 text-indigo-300 rounded-md border border-indigo-500/20 hover:bg-indigo-500/25 transition-colors"
          >
            重试
          </button>
        </div>
      </div>
    );
  }

  if (loading && !graph) {
    return (
      <div className="h-screen flex items-center justify-center bg-[var(--bg-base)]">
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          className="text-center"
        >
          <div className="w-8 h-8 border-2 border-indigo-500/30 border-t-indigo-500 rounded-full animate-spin mx-auto mb-3" />
          <p className="text-sm text-zinc-500">加载世界状态...</p>
        </motion.div>
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col bg-[var(--bg-base)]">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <div className="flex-1 flex flex-col overflow-hidden">
          <Toolbar />
          <div className="flex-1 flex overflow-hidden">
            <GraphCanvas />
            <Inspector />
          </div>
        </div>
      </div>
    </div>
  );
}
