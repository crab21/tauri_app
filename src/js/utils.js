export const clusterScopedKinds = new Set([
  "pv", "persistentvolume",
  "sc", "storageclass",
  "crd", "customresourcedefinition",
  "node", "namespace", "ns",
  "clusterrole", "clusterrolebinding"
]);

export function logsSupportedKind(kind) {
  const k = (kind || "").toLowerCase();
  return k === "pod" || k === "pods";
}

export function formatAge(iso) {
  if (!iso) return "";
  const created = new Date(iso);
  const now = new Date();
  const diff = Math.max(0, now - created);
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days}d`;
  if (hours > 0) return `${hours}h`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

export function getItemKey(kind, it) {
  const name = it?.metadata?.name || "";
  const ns = it?.metadata?.namespace || "";
  const isCluster = clusterScopedKinds.has((kind || "").toLowerCase());
  return isCluster ? name : `${ns}/${name}`;
}


