import type { ProjectInfo, PhasesResponse, GraphResponse } from './types';

async function fetchJson<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
  return res.json();
}

export async function listProjects(): Promise<ProjectInfo[]> {
  return fetchJson('/api/projects');
}

export async function getPhases(project: string): Promise<PhasesResponse> {
  return fetchJson(`/api/projects/${encodeURIComponent(project)}/phases`);
}

export async function getGraph(project: string, phase: string): Promise<GraphResponse> {
  return fetchJson(
    `/api/projects/${encodeURIComponent(project)}/graph/${encodeURIComponent(phase)}`
  );
}
