// API response types matching the Rust backend

export interface ProjectInfo {
  id: string;
  has_build: boolean;
  current_phase: string | null;
  phase_count: number;
}

export interface PhasesResponse {
  current: string | null;
  plan: string[];
  phases: Record<string, PhaseInfo>;
}

export interface PhaseInfo {
  status: string;
  beats: number;
  words: number;
  effects: number;
}

export interface GraphResponse {
  nodes: GraphNode[];
  edges: GraphEdge[];
  secrets: SecretNode[];
}

export interface GraphNode {
  id: string;
  type: 'character' | 'location' | 'faction';
  name: string;
  description?: string;
  traits?: string[];
  beliefs?: string[];
  desires?: string[];
  location?: string;
  inventory?: string[];
  tags?: string[];
  properties?: string[];
  connections?: string[];
  alignment?: string;
  members?: string[];
  rivals?: string[];
  leader?: string;
  goals?: GoalBrief[];
}

export interface GoalBrief {
  id: string;
  want: string;
  status: string;
  conflicts_with: string[];
}

export interface GraphEdge {
  source: string;
  target: string;
  type: string;
  label?: string;
  trust?: number;
  affinity?: number;
  respect?: number;
}

export interface SecretNode {
  id: string;
  content: string;
  known_by: string[];
  revealed_to_reader: boolean;
  dramatic_function?: string;
}
