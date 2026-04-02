import { create } from 'zustand';
import type { ProjectInfo, PhasesResponse, GraphResponse, GraphNode } from '../lib/types';
import { listProjects, getPhases, getGraph } from '../lib/api';

type SecretMode = 'reader' | 'omniscient';

interface AppState {
  // Data
  projects: ProjectInfo[];
  currentProject: string | null;
  phases: PhasesResponse | null;
  currentPhase: string | null;
  graph: GraphResponse | null;
  loading: boolean;
  error: string | null;

  // UI
  selectedNode: GraphNode | null;
  secretMode: SecretMode;
  activeEdgeTypes: Set<string>;

  // Actions
  init: () => Promise<void>;
  selectProject: (project: string) => Promise<void>;
  selectPhase: (phase: string) => Promise<void>;
  selectNode: (node: GraphNode | null) => void;
  setSecretMode: (mode: SecretMode) => void;
  toggleEdgeType: (type: string) => void;
}

export const useStore = create<AppState>((set, get) => ({
  projects: [],
  currentProject: null,
  phases: null,
  currentPhase: null,
  graph: null,
  loading: false,
  error: null,
  selectedNode: null,
  secretMode: 'reader',
  activeEdgeTypes: new Set([
    'relationship', 'location', 'membership', 'rivalry', 'connection', 'goal_conflict',
  ]),

  init: async () => {
    try {
      set({ loading: true, error: null });
      const projects = await listProjects();
      set({ projects });
      if (projects.length > 0) {
        // Pick the first project that has a build.
        const first = projects.find((p) => p.has_build) || projects[0];
        await get().selectProject(first.id);
      }
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loading: false });
    }
  },

  selectProject: async (project: string) => {
    try {
      set({ loading: true, currentProject: project, selectedNode: null, graph: null });
      const phases = await getPhases(project);
      const phaseIds = phases.plan.length > 0
        ? phases.plan
        : Object.keys(phases.phases).sort();
      const current = phases.current || phaseIds[0] || null;
      set({ phases, currentPhase: current });
      if (current) {
        const graph = await getGraph(project, current);
        set({ graph });
      }
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loading: false });
    }
  },

  selectPhase: async (phase: string) => {
    const project = get().currentProject;
    if (!project) return;
    try {
      set({ loading: true, currentPhase: phase, selectedNode: null });
      const graph = await getGraph(project, phase);
      set({ graph });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ loading: false });
    }
  },

  selectNode: (node) => set({ selectedNode: node }),
  setSecretMode: (mode) => set({ secretMode: mode }),
  toggleEdgeType: (type) => {
    const current = get().activeEdgeTypes;
    const next = new Set(current);
    if (next.has(type)) next.delete(type);
    else next.add(type);
    set({ activeEdgeTypes: next });
  },
}));
