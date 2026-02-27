/**
 * Svelte store that mirrors the Rust model graph.
 * All writes go through Tauri commands; this store is the read-side cache.
 * Components never call invoke() directly — they use the actions below.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writable, derived, get } from "svelte/store";
import type { Project, Node, Edge, Diagram, ValidationIssue } from "$lib/types";

// ── Raw stores ────────────────────────────────────────────────────────────────

export const currentProject = writable<Project | null>(null);
export const nodes = writable<Node[]>([]);
export const edges = writable<Edge[]>([]);
export const diagrams = writable<Diagram[]>([]);
export const validationIssues = writable<ValidationIssue[]>([]);
export const aiAvailable = writable<boolean>(false);
export const readOnly = writable<boolean>(false);
const requirementHistoryByNode = writable<
  Map<string, RequirementHistoryEntry[]>
>(new Map());

export type AccessRole = "admin" | "system" | "subsystem" | "viewer";

export type AccessProfile = {
  id: string;
  name: string;
  role: AccessRole;
  subsystem_id?: string;
};

export type AccessControlState = {
  enabled: boolean;
  active_profile_id: string;
  profiles: AccessProfile[];
  updated_at: string;
};

const ACCESS_SETTING_KEY = "access.control.v1";
const ACCESS_ROLE_OVERRIDES_KEY = "access.role_overrides.v1";

export type AuthRoleOverride = {
  user_id?: string;
  email?: string;
  role: AccessRole;
  subsystem_id?: string;
  updated_at: string;
};
const DEFAULT_ACCESS_PROFILES: AccessProfile[] = [
  { id: "admin", name: "Admin", role: "admin" },
  { id: "system", name: "System Engineer", role: "system" },
  { id: "subsystem", name: "Subsystem Owner", role: "subsystem" },
  { id: "viewer", name: "Viewer", role: "viewer" },
];

function defaultAccessControlState(nodesList: Node[]): AccessControlState {
  const firstSubsystem = nodesList.find(
    (n) =>
      n.kind === "block" && !(n.meta as Record<string, unknown>)?.system_root,
  );
  return {
    enabled: false,
    active_profile_id: "admin",
    profiles: DEFAULT_ACCESS_PROFILES.map((p) =>
      p.role === "subsystem"
        ? { ...p, subsystem_id: firstSubsystem?.id ?? undefined }
        : { ...p },
    ),
    updated_at: new Date().toISOString(),
  };
}

function normalizeAccessControlState(
  raw: unknown,
  nodesList: Node[],
): AccessControlState {
  const fallback = defaultAccessControlState(nodesList);
  if (!raw || typeof raw !== "object") return fallback;

  const obj = raw as Record<string, unknown>;
  const rawProfiles = Array.isArray(obj.profiles) ? obj.profiles : [];
  const byId = new Map<string, AccessProfile>();

  for (const entry of rawProfiles) {
    if (!entry || typeof entry !== "object") continue;
    const e = entry as Record<string, unknown>;
    const id = String(e.id ?? "").trim();
    const role = String(e.role ?? "").trim() as AccessRole;
    if (!id || !["admin", "system", "subsystem", "viewer"].includes(role))
      continue;
    byId.set(id, {
      id,
      role,
      name: String(e.name ?? id),
      subsystem_id:
        typeof e.subsystem_id === "string" && e.subsystem_id.trim()
          ? e.subsystem_id.trim()
          : undefined,
    });
  }

  const profiles = DEFAULT_ACCESS_PROFILES.map((base) => {
    const incoming = byId.get(base.id);
    if (!incoming || incoming.role !== base.role) return { ...base };
    return {
      ...base,
      name: incoming.name || base.name,
      subsystem_id: incoming.subsystem_id,
    };
  });

  const activeId = String(obj.active_profile_id ?? "").trim();
  const hasActive = profiles.some((p) => p.id === activeId);

  return {
    enabled: Boolean(obj.enabled),
    active_profile_id: hasActive ? activeId : "admin",
    profiles,
    updated_at:
      typeof obj.updated_at === "string" && obj.updated_at.trim()
        ? obj.updated_at
        : new Date().toISOString(),
  };
}

function activeProfileFromState(state: AccessControlState): AccessProfile {
  return (
    state.profiles.find((p) => p.id === state.active_profile_id) ??
    state.profiles[0] ??
    DEFAULT_ACCESS_PROFILES[0]
  );
}

export const accessControl = writable<AccessControlState>(
  defaultAccessControlState([]),
);
export const activeAccessProfile = derived(accessControl, ($accessControl) =>
  activeProfileFromState($accessControl),
);
export const authRoleOverrides = writable<AuthRoleOverride[]>([]);

function normalizeEmail(value: string | undefined): string | undefined {
  if (!value) return undefined;
  const trimmed = value.trim().toLowerCase();
  return trimmed || undefined;
}

function normalizeAuthRoleOverrides(
  raw: unknown,
  nodesList: Node[],
): AuthRoleOverride[] {
  const obj = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : null;
  const rawList = Array.isArray(obj?.overrides)
    ? obj?.overrides
    : Array.isArray(raw)
      ? raw
      : [];
  const subsystemIds = new Set(
    nodesList
      .filter(
        (n) =>
          n.kind === "block" &&
          !(n.meta as Record<string, unknown>)?.system_root,
      )
      .map((n) => n.id),
  );
  const dedupe = new Map<string, AuthRoleOverride>();

  for (const entry of rawList) {
    if (!entry || typeof entry !== "object") continue;
    const e = entry as Record<string, unknown>;
    const role = String(e.role ?? "").trim() as AccessRole;
    if (!["admin", "system", "subsystem", "viewer"].includes(role)) continue;

    const userId =
      typeof e.user_id === "string" && e.user_id.trim()
        ? e.user_id.trim()
        : undefined;
    const email = normalizeEmail(
      typeof e.email === "string" ? e.email : undefined,
    );
    if (!userId && !email) continue;

    const subsystemId =
      typeof e.subsystem_id === "string" &&
      e.subsystem_id.trim() &&
      subsystemIds.has(e.subsystem_id.trim())
        ? e.subsystem_id.trim()
        : undefined;
    const key = userId ? `id:${userId}` : `email:${email}`;

    dedupe.set(key, {
      user_id: userId,
      email,
      role,
      subsystem_id: role === "subsystem" ? subsystemId : undefined,
      updated_at:
        typeof e.updated_at === "string" && e.updated_at.trim()
          ? e.updated_at
          : new Date().toISOString(),
    });
  }

  return [...dedupe.values()];
}

type RequirementSnapshot = {
  req_id: string;
  name: string;
  text: string;
  rationale: string;
  priority: string;
  status: string;
  verification_method: string;
  source: string;
  allocations: string[];
  description: string;
};

export type RequirementHistoryEntry = {
  id: string;
  project_id: string;
  node_id: string;
  ts: string;
  actor: string;
  source: string;
  prev: RequirementSnapshot;
  next: RequirementSnapshot;
};

const REQ_HISTORY_LIMIT = 20;

export function getReadOnlySetting(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem("app.readOnly") === "1";
}

export function setReadOnly(value: boolean) {
  readOnly.set(value);
  if (typeof localStorage !== "undefined") {
    localStorage.setItem("app.readOnly", value ? "1" : "0");
  }
}

function subsystemNameFromId(
  subsystemId: string | undefined,
  nodesList: Node[],
): string | null {
  if (!subsystemId) return null;
  const node = nodesList.find(
    (n) => n.id === subsystemId && n.kind === "block",
  );
  return node?.name?.trim() || null;
}

function requirementAllocations(node: Node): string[] {
  if (node.kind !== "requirement") return [];
  const raw = (node.data as { allocations?: string[] }).allocations ?? [];
  return raw.map((v) => v.trim()).filter(Boolean);
}

function allocationMatchesSubsystem(
  allocation: string,
  subsystemId: string | undefined,
  subsystemName: string | null,
): boolean {
  const normalized = allocation.trim().toLowerCase();
  if (!normalized) return false;
  if (subsystemId && normalized === subsystemId.trim().toLowerCase())
    return true;
  if (subsystemName && normalized === subsystemName.trim().toLowerCase())
    return true;
  return false;
}

function isSystemLevelAllocations(allocations: string[]): boolean {
  if (allocations.length === 0) return true;
  return allocations.every((a) => {
    const key = a.trim().toLowerCase();
    return (
      !key ||
      key === "system" ||
      key === "system level" ||
      key === "system-level"
    );
  });
}

function canWriteBaseline(): boolean {
  if (get(readOnly)) return false;
  const acl = get(accessControl);
  if (!acl.enabled) return true;
  return activeProfileFromState(acl).role !== "viewer";
}

function stampNodeAuditMeta(node: Node): Node {
  const meta = { ...(node.meta ?? {}) } as Record<string, unknown>;
  if (!meta.actor) {
    const acl = get(accessControl);
    const profile = activeProfileFromState(acl);
    meta.actor = acl.enabled ? profile.id : "system";
  }
  if (!meta.change_source) {
    meta.change_source = "manual";
  }
  return { ...node, meta };
}

export function canManageAccessControl(): boolean {
  if (get(readOnly)) return false;
  const acl = get(accessControl);
  if (!acl.enabled) return true;
  return activeProfileFromState(acl).role === "admin";
}

export function canViewNode(node: Node): boolean {
  const acl = get(accessControl);
  if (!acl.enabled) return true;
  const profile = activeProfileFromState(acl);
  if (
    profile.role === "admin" ||
    profile.role === "system" ||
    profile.role === "viewer"
  ) {
    return true;
  }

  const nodesList = get(nodes);
  const scopedName = subsystemNameFromId(profile.subsystem_id, nodesList);
  if (node.kind === "block") {
    if ((node.meta as Record<string, unknown>)?.system_root) return true;
    return profile.subsystem_id === node.id;
  }
  if (node.kind !== "requirement") return false;

  const allocs = requirementAllocations(node);
  if (allocs.length === 0) return false;
  return allocs.some((a) =>
    allocationMatchesSubsystem(a, profile.subsystem_id, scopedName),
  );
}

export function canEditNode(node: Node): boolean {
  if (!canWriteBaseline()) return false;
  const acl = get(accessControl);
  if (!acl.enabled) return true;

  const profile = activeProfileFromState(acl);
  if (profile.role === "admin" || profile.role === "system") return true;
  if (profile.role === "viewer") return false;

  const nodesList = get(nodes);
  const scopedName = subsystemNameFromId(profile.subsystem_id, nodesList);

  if (node.kind === "block") {
    return profile.subsystem_id === node.id;
  }

  if (node.kind !== "requirement") return false;
  const allocs = requirementAllocations(node);
  if (allocs.length === 0) return false;
  return (
    allocs.every((a) =>
      allocationMatchesSubsystem(a, profile.subsystem_id, scopedName),
    ) &&
    allocs.some((a) =>
      allocationMatchesSubsystem(a, profile.subsystem_id, scopedName),
    )
  );
}

export function canEditEdge(edge: Edge): boolean {
  if (!canWriteBaseline()) return false;
  const acl = get(accessControl);
  if (!acl.enabled) return true;
  const role = activeProfileFromState(acl).role;
  return role === "admin" || role === "system";
}

export function canCreateSubsystem(): boolean {
  if (!canWriteBaseline()) return false;
  const acl = get(accessControl);
  if (!acl.enabled) return true;
  const role = activeProfileFromState(acl).role;
  return role === "admin" || role === "system";
}

export function canCreateRequirementForAllocations(
  allocations?: string[] | null,
): boolean {
  if (!canWriteBaseline()) return false;
  const acl = get(accessControl);
  if (!acl.enabled) return true;
  const profile = activeProfileFromState(acl);
  if (profile.role === "admin" || profile.role === "system") return true;
  if (profile.role === "viewer") return false;

  const normalized = (allocations ?? []).map((v) => v.trim()).filter(Boolean);
  if (isSystemLevelAllocations(normalized)) return false;
  const scopedName = subsystemNameFromId(profile.subsystem_id, get(nodes));
  return (
    normalized.every((a) =>
      allocationMatchesSubsystem(a, profile.subsystem_id, scopedName),
    ) &&
    normalized.some((a) =>
      allocationMatchesSubsystem(a, profile.subsystem_id, scopedName),
    )
  );
}

async function persistAccessControl(projectId?: string) {
  const pid = projectId ?? get(currentProject)?.id;
  if (!pid) return;
  const value = JSON.stringify(get(accessControl));
  await invoke("set_setting", {
    key: ACCESS_SETTING_KEY,
    value,
    projectId: pid,
  }).catch(() => undefined);
}

async function persistAuthRoleOverrides(projectId?: string) {
  const pid = projectId ?? get(currentProject)?.id;
  if (!pid) return;
  const value = JSON.stringify({
    overrides: get(authRoleOverrides),
    updated_at: new Date().toISOString(),
  });
  await invoke("set_setting", {
    key: ACCESS_ROLE_OVERRIDES_KEY,
    value,
    projectId: pid,
  }).catch(() => undefined);
}

async function loadAccessControl(projectId: string, projectNodes: Node[]) {
  const raw = await invoke<string | null>("get_setting", {
    key: ACCESS_SETTING_KEY,
    projectId,
  }).catch(() => null);
  const parsed = (() => {
    if (!raw) return null;
    try {
      return JSON.parse(raw);
    } catch {
      return null;
    }
  })();

  const normalized = normalizeAccessControlState(parsed, projectNodes);
  accessControl.set(normalized);
}

async function loadAuthRoleOverrides(projectId: string, projectNodes: Node[]) {
  const raw = await invoke<string | null>("get_setting", {
    key: ACCESS_ROLE_OVERRIDES_KEY,
    projectId,
  }).catch(() => null);
  const parsed = (() => {
    if (!raw) return null;
    try {
      return JSON.parse(raw);
    } catch {
      return null;
    }
  })();
  authRoleOverrides.set(normalizeAuthRoleOverrides(parsed, projectNodes));
}

export async function setAccessControlEnabled(value: boolean) {
  accessControl.update((state) => ({
    ...state,
    enabled: value,
    updated_at: new Date().toISOString(),
  }));
  await persistAccessControl();
}

export async function setActiveAccessProfile(profileId: string) {
  accessControl.update((state) => {
    const has = state.profiles.some((p) => p.id === profileId);
    return {
      ...state,
      active_profile_id: has ? profileId : state.active_profile_id,
      updated_at: new Date().toISOString(),
    };
  });
  await persistAccessControl();
}

// Update the active profile in-memory only (no persistence).
// Use this for authenticated session-based role mapping so one user does not
// overwrite project ACL defaults for everyone else.
export function setActiveAccessProfileSession(profileId: string) {
  accessControl.update((state) => {
    const has = state.profiles.some((p) => p.id === profileId);
    return {
      ...state,
      active_profile_id: has ? profileId : state.active_profile_id,
      updated_at: new Date().toISOString(),
    };
  });
}

export async function setSubsystemScope(subsystemId?: string) {
  accessControl.update((state) => ({
    ...state,
    profiles: state.profiles.map((p) =>
      p.role === "subsystem"
        ? { ...p, subsystem_id: subsystemId || undefined }
        : p,
    ),
    updated_at: new Date().toISOString(),
  }));
  await persistAccessControl();
}

// Update the subsystem scope for the `subsystem` profile in-memory only.
// This does NOT persist the change to project settings. Use this when a
// local/session-only actor (a subsystem user) needs to pick which subsystem
// they are acting as without modifying the project-wide ACL configuration.
export function setSubsystemScopeSession(subsystemId?: string) {
  accessControl.update((state) => ({
    ...state,
    profiles: state.profiles.map((p) =>
      p.role === "subsystem"
        ? { ...p, subsystem_id: subsystemId || undefined }
        : p,
    ),
    updated_at: new Date().toISOString(),
  }));
}

export function getAuthRoleOverride(
  userId?: string,
  email?: string,
): AuthRoleOverride | null {
  const normalizedEmail = normalizeEmail(email);
  const overrides = get(authRoleOverrides);
  if (userId) {
    const byId = overrides.find((o) => o.user_id === userId);
    if (byId) return byId;
  }
  if (normalizedEmail) {
    const byEmail = overrides.find((o) => normalizeEmail(o.email) === normalizedEmail);
    if (byEmail) return byEmail;
  }
  return null;
}

export async function upsertAuthRoleOverride(input: {
  user_id?: string;
  email?: string;
  role: AccessRole;
  subsystem_id?: string;
}) {
  const userId = input.user_id?.trim() || undefined;
  const email = normalizeEmail(input.email);
  if (!userId && !email) return;

  const role = input.role;
  if (!["admin", "system", "subsystem", "viewer"].includes(role)) return;

  const normalized: AuthRoleOverride = {
    user_id: userId,
    email,
    role,
    subsystem_id: role === "subsystem" ? input.subsystem_id?.trim() || undefined : undefined,
    updated_at: new Date().toISOString(),
  };

  authRoleOverrides.update((list) => {
    const next = list.filter((o) => {
      const sameId = Boolean(userId && o.user_id === userId);
      const sameEmail = Boolean(email && normalizeEmail(o.email) === email);
      return !(sameId || sameEmail);
    });
    next.push(normalized);
    return next;
  });
  await persistAuthRoleOverrides();
}

export async function removeAuthRoleOverride(input: {
  user_id?: string;
  email?: string;
}) {
  const userId = input.user_id?.trim() || undefined;
  const email = normalizeEmail(input.email);
  if (!userId && !email) return;

  authRoleOverrides.update((list) =>
    list.filter((o) => {
      const sameId = Boolean(userId && o.user_id === userId);
      const sameEmail = Boolean(email && normalizeEmail(o.email) === email);
      return !(sameId || sameEmail);
    }),
  );
  await persistAuthRoleOverrides();
}

// ── Derived views ─────────────────────────────────────────────────────────────

export const nodeById = derived(nodes, ($nodes) => {
  const map = new Map<string, Node>();
  for (const n of $nodes) map.set(n.id, n);
  return map;
});

export const nodesByKind = derived(nodes, ($nodes) => {
  const map = new Map<string, Node[]>();
  for (const n of $nodes) {
    const list = map.get(n.kind) ?? [];
    list.push(n);
    map.set(n.kind, list);
  }
  return map;
});

export const edgesForNode = derived(edges, ($edges) => {
  return (nodeId: string) =>
    $edges.filter((e) => e.source_id === nodeId || e.target_id === nodeId);
});

export const issuesByNode = derived(validationIssues, ($issues) => {
  const map = new Map<string, ValidationIssue[]>();
  for (const issue of $issues) {
    if (issue.node_id) {
      const list = map.get(issue.node_id) ?? [];
      list.push(issue);
      map.set(issue.node_id, list);
    }
  }
  return map;
});

// ── Actions ───────────────────────────────────────────────────────────────────

export async function loadProject(projectId: string) {
  const [project, projectNodes, projectDiagrams, available] = await Promise.all(
    [
      invoke<Project>("get_project", { id: projectId }),
      invoke<Node[]>("list_nodes", { projectId }),
      invoke<Diagram[]>("list_diagrams", { projectId }),
      invoke<boolean>("ai_available"),
    ],
  );

  // Load all edges touching these nodes
  const edgeArrays = await Promise.all(
    projectNodes.map((n) => invoke<Edge[]>("edges_for_node", { nodeId: n.id })),
  );
  const allEdges = dedupeById(edgeArrays.flat());
  const historyPairs = await Promise.all(
    projectNodes
      .filter((n) => n.kind === "requirement")
      .map(async (n) => {
        const history = await invoke<RequirementHistoryEntry[]>(
          "list_requirement_history",
          {
            nodeId: n.id,
            limit: REQ_HISTORY_LIMIT,
          },
        ).catch(() => []);
        return [n.id, history] as const;
      }),
  );

  currentProject.set(project);
  requirementHistoryByNode.set(new Map(historyPairs));
  nodes.set(projectNodes);
  edges.set(allEdges);
  diagrams.set(projectDiagrams);
  aiAvailable.set(available);
  await loadAccessControl(projectId, projectNodes);
  await loadAuthRoleOverrides(projectId, projectNodes);

  await refreshValidation(projectId);
}

export async function saveNode(node: Node) {
  if (!canEditNode(node)) return;
  const audited = stampNodeAuditMeta(node);
  // Optimistic update: update the store immediately so all views react without
  // waiting for the backend round-trip.
  nodes.update((ns) => {
    const idx = ns.findIndex((n) => n.id === audited.id);
    if (idx >= 0) {
      const next = [...ns];
      next[idx] = audited;
      return next;
    }
    return [...ns, audited];
  });
  await invoke("upsert_node", { node: audited });
  if (audited.kind === "requirement") {
    await refreshRequirementHistory(audited.id);
  }
}

export async function removeNode(nodeId: string) {
  const removed = get(nodes).find((n) => n.id === nodeId) ?? null;
  if (!removed || !canEditNode(removed)) return;
  // Optimistic update: remove from UI immediately before backend confirms
  nodes.update((ns) => ns.filter((n) => n.id !== nodeId));
  edges.update((es) =>
    es.filter((e) => e.source_id !== nodeId && e.target_id !== nodeId),
  );
  if (removed?.kind === "requirement") {
    requirementHistoryByNode.update((m) => {
      const next = new Map(m);
      next.delete(nodeId);
      return next;
    });
  }
  await invoke("delete_node", { id: nodeId });
}

export async function saveEdge(edge: Edge) {
  if (!canEditEdge(edge)) return;
  // Optimistic update so diagrams/lists react immediately
  edges.update((es) => {
    const idx = es.findIndex((e) => e.id === edge.id);
    if (idx >= 0) {
      const next = [...es];
      next[idx] = edge;
      return next;
    }
    return [...es, edge];
  });
  await invoke("upsert_edge", { edge });
}

export async function removeEdge(edgeId: string) {
  const edge = get(edges).find((e) => e.id === edgeId) ?? null;
  if (!edge || !canEditEdge(edge)) return;
  // Optimistic update
  edges.update((es) => es.filter((e) => e.id !== edgeId));
  await invoke("delete_edge", { id: edgeId });
}

export function getRequirementHistory(
  nodeId: string,
): RequirementHistoryEntry[] {
  return get(requirementHistoryByNode).get(nodeId) ?? [];
}

async function refreshRequirementHistory(nodeId: string) {
  const history = await invoke<RequirementHistoryEntry[]>(
    "list_requirement_history",
    {
      nodeId,
      limit: REQ_HISTORY_LIMIT,
    },
  ).catch(() => []);
  requirementHistoryByNode.update((m) => {
    const next = new Map(m);
    next.set(nodeId, history);
    return next;
  });
}

async function refreshValidation(projectId: string) {
  const issues = await invoke<ValidationIssue[]>("validate_model", {
    projectId,
  });
  validationIssues.set(issues);
}

// ── Backend event listeners ───────────────────────────────────────────────────

export function initEventListeners() {
  listen("model:changed", () => {
    const p = get(currentProject);
    if (p) refreshValidation(p.id);
  });

  listen<ValidationIssue[]>("validation:updated", (event) => {
    validationIssues.set(event.payload);
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function dedupeById<T extends { id: string }>(items: T[]): T[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    if (seen.has(item.id)) return false;
    seen.add(item.id);
    return true;
  });
}
