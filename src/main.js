import { getK8sObject, getK8sResources, getPodLogs } from "./js/api.js";
import { clusterScopedKinds, formatAge, getItemKey, logsSupportedKind } from "./js/utils.js";
import { initViewer, openViewer, renderJsonTree } from "./js/viewer.js";
let refreshTimerId = null;
let countdownTimerId = null;
const DEFAULT_REFRESH_MS = 5000;
let refreshIntervalMs = DEFAULT_REFRESH_MS;
let lastRenderedKind = null;
let nextRefreshAt = 0;

function showLoading(isLoading) {
  const el = document.getElementById("loading-indicator");
  if (!el) return;
  if (isLoading) {
    el.classList.add("on");
  } else {
    el.classList.remove("on");
  }
}

function setActiveKind(kind) {
  const items = document.querySelectorAll("#resource-list li");
  items.forEach((li) => {
    if (li.dataset.kind === kind) {
      li.classList.add("active");
    } else {
      li.classList.remove("active");
    }
  });
  const titleEl = document.getElementById("current-kind");
  if (titleEl) titleEl.textContent = kind;
}

function setStatus(message, isError = false) {
  const statusEl = document.getElementById("status");
  if (!statusEl) return;
  statusEl.textContent = message || "";
  statusEl.style.display = message ? "block" : "none";
  statusEl.classList.toggle("error", !!isError);
}

 

function getSelectedNamespace() {
  const sel = document.getElementById("namespace-select");
  if (!sel) return "all";
  return sel.value || "all";
}

 

function ensureBaseTable(kind) {
  const container = document.getElementById("table-container");
  if (!container) return null;
  let table = container.querySelector("table.k8s-table");
  if (!table || lastRenderedKind !== kind) {
    container.innerHTML = "";
    table = document.createElement("table");
    table.className = "k8s-table";
    const thead = document.createElement("thead");
    thead.innerHTML = `
      <tr>
        <th>Namespace</th>
        <th>Name</th>
        <th>Age</th>
        <th style="width:160px">Actions</th>
      </tr>
    `;
    const tbody = document.createElement("tbody");
    table.appendChild(thead);
    table.appendChild(tbody);
    container.appendChild(table);
    lastRenderedKind = kind;
  }
  return table;
}

function renderOrUpdateTable(kind, listJson) {
  const items = (listJson && listJson.items) || [];
  const container = document.getElementById("table-container");
  if (!container) return;
  if (items.length === 0) {
    // Create base table if not exists, then clear tbody and show empty state
    const table = ensureBaseTable(kind);
    if (!table) return;
    const tbody = table.tBodies[0];
    while (tbody.firstChild) tbody.removeChild(tbody.firstChild);
    const empty = document.createElement("tr");
    empty.innerHTML = `<td colspan="3"><div class="empty">无数据</div></td>`;
    tbody.appendChild(empty);
    return;
  }

  const table = ensureBaseTable(kind);
  if (!table) return;
  const tbody = table.tBodies[0];

  // Build maps
  const desired = items
    .map((it) => ({ key: getItemKey(kind, it), ns: it?.metadata?.namespace || "", name: it?.metadata?.name || "", age: formatAge(it?.metadata?.creationTimestamp), it }))
    .sort((a, b) => (a.ns === b.ns ? a.name.localeCompare(b.name) : a.ns.localeCompare(b.ns)));
  const desiredMap = new Map(desired.map((d) => [d.key, d]));

  // Index existing rows
  const existingRows = new Map();
  Array.from(tbody.querySelectorAll("tr[data-key]")).forEach((tr) => {
    const key = tr.getAttribute("data-key");
    existingRows.set(key, tr);
  });

  // Remove rows that no longer exist
  existingRows.forEach((tr, key) => {
    if (!desiredMap.has(key)) {
      tr.style.opacity = "0";
      setTimeout(() => tr.remove(), 180);
      existingRows.delete(key);
    }
  });

  // Upsert rows and update cells
  desired.forEach((d) => {
    let tr = existingRows.get(d.key);
    if (!tr) {
      tr = document.createElement("tr");
      tr.setAttribute("data-key", d.key);
      tr.style.opacity = "0";
      const tdNs = document.createElement("td");
      tdNs.className = "cell-ns";
      const tdName = document.createElement("td");
      tdName.className = "cell-name";
      tdName.innerHTML = `<span class="name-text"></span>`;
      const tdAge = document.createElement("td");
      tdAge.className = "cell-age";
      const tdActions = document.createElement("td");
      tdActions.className = "cell-actions";
      let actionsHtml = `<button class="btn-describe" title="Describe (YAML)">Describe</button>`;
      if (logsSupportedKind(kind)) {
        actionsHtml += ` <button class="btn-logs" title="Logs">Logs</button>`;
      }
      tdActions.innerHTML = actionsHtml;
      tr.appendChild(tdNs);
      tr.appendChild(tdName);
      tr.appendChild(tdAge);
      tr.appendChild(tdActions);
      tbody.appendChild(tr);
      requestAnimationFrame(() => {
        tr.style.opacity = "1";
      });
      existingRows.set(d.key, tr);
    }
    const tdNs = tr.querySelector(".cell-ns");
    const tdName = tr.querySelector(".cell-name");
    const tdAge = tr.querySelector(".cell-age");
    if (tdNs && tdNs.textContent !== d.ns) tdNs.textContent = d.ns;
    if (tdName) {
      let span = tdName.querySelector(".name-text");
      if (!span) {
        span = document.createElement("span");
        span.className = "name-text";
        tdName.appendChild(span);
      }
      if (span.textContent !== d.name) {
        span.textContent = d.name;
        span.title = d.name;
      }
    }
    if (tdAge && tdAge.textContent !== d.age) tdAge.textContent = d.age;
  });

  // Reorder rows to match sorted desired order with minimal moves (reverse insertBefore)
  let nextRef = null;
  for (let i = desired.length - 1; i >= 0; i--) {
    const key = desired[i].key;
    const row = existingRows.get(key);
    if (!row) continue;
    if (row.nextSibling !== nextRef) {
      tbody.insertBefore(row, nextRef);
    }
    nextRef = row;
  }
}

function getActiveKind() {
  const active = document.querySelector("#resource-list li.active");
  return (active && active.dataset.kind) || "pod";
}

function updateCountdownOnce() {
  const el = document.getElementById("refresh-countdown");
  if (!el) return;
  if (!nextRefreshAt) {
    el.textContent = "";
    return;
  }
  const remainingMs = Math.max(0, nextRefreshAt - Date.now());
  const remainingSec = Math.ceil(remainingMs / 1000);
  el.textContent = String(remainingSec);
}

function startCountdown() {
  if (countdownTimerId) {
    clearInterval(countdownTimerId);
    countdownTimerId = null;
  }
  updateCountdownOnce();
  countdownTimerId = setInterval(updateCountdownOnce, 1000);
}

function scheduleAutoRefresh() {
  if (refreshTimerId) {
    clearInterval(refreshTimerId);
    refreshTimerId = null;
  }
  nextRefreshAt = Date.now() + refreshIntervalMs;
  startCountdown();
  refreshTimerId = setInterval(() => {
    const kind = getActiveKind();
    loadKind(kind);
    nextRefreshAt = Date.now() + refreshIntervalMs;
    updateCountdownOnce();
  }, refreshIntervalMs);
}

async function loadNamespaces() {
  try {
    const jsonStr = await getK8sResources("ns", null);
    const data = JSON.parse(jsonStr);
    const sel = document.getElementById("namespace-select");
    if (!sel) return;
    const current = sel.value || "all";
    sel.innerHTML = "";
    const optAll = document.createElement("option");
    optAll.value = "all";
    optAll.textContent = "All namespaces";
    sel.appendChild(optAll);
    (data.items || []).forEach((it) => {
      const name = it?.metadata?.name;
      if (!name) return;
      const opt = document.createElement("option");
      opt.value = name;
      opt.textContent = name;
      sel.appendChild(opt);
    });
    sel.value = current; // preserve selection if exists
  } catch (e) {
    // best-effort; ignore errors
  }
}

async function loadKind(kind) {
  setActiveKind(kind);
  showLoading(true);
  try {
    const ns = getSelectedNamespace();
    const useNamespace = clusterScopedKinds.has(kind.toLowerCase()) ? null : (ns === "all" ? null : ns);
    const jsonStr = await getK8sResources(kind, useNamespace);
    const data = JSON.parse(jsonStr);
    setStatus("");
    renderOrUpdateTable(kind, data);
  } catch (err) {
    setStatus(`加载失败: ${String(err)}`, true);
  } finally {
    showLoading(false);
  }
}

// viewer helpers now imported from /js/viewer.js

async function handleDescribe(kind, ns, name) {
  try {
    showLoading(true);
    const useNs = clusterScopedKinds.has((kind || "").toLowerCase()) ? null : ns;
    const [yaml, jsonStr] = await Promise.all([
      getK8sObject(kind, name, useNs, "yaml"),
      getK8sObject(kind, name, useNs, "json")
    ]);
    openViewer(`${kind}/${name}`, yaml, "describe");
    renderJsonTree(jsonStr);
  } catch (e) {
    openViewer("Error", String(e));
  } finally {
    showLoading(false);
  }
}

async function handleLogs(kind, ns, name) {
  if ((kind || "").toLowerCase() !== "pod") return;
  if (!ns || ns === "all") {
    openViewer("Error", "Namespace is required for pod logs");
    return;
  }
  try {
    showLoading(true);
    const logs = await getPodLogs(name, ns, 500);
    openViewer(`pod/${name} logs`, logs, "logs");
  } catch (e) {
    openViewer("Error", String(e));
  } finally {
    showLoading(false);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  loadNamespaces();
  const list = document.getElementById("resource-list");
  if (list) {
    list.addEventListener("click", (e) => {
      const li = e.target && e.target.closest("li[data-kind]");
      if (!li) return;
      const kind = li.dataset.kind;
      loadKind(kind);
      scheduleAutoRefresh();
    });
  }
  const nsSel = document.getElementById("namespace-select");
  if (nsSel) {
    nsSel.addEventListener("change", () => {
      const kind = getActiveKind();
      loadKind(kind);
      scheduleAutoRefresh();
    });
  }
  const refreshBtn = document.getElementById("refresh-btn");
  if (refreshBtn) {
    refreshBtn.addEventListener("click", () => {
      loadNamespaces(); // refresh namespaces too
      const kind = getActiveKind();
      loadKind(kind);
      scheduleAutoRefresh(); // reset timer and countdown
    });
  }
  const autoSel = document.getElementById("auto-refresh-select");
  if (autoSel) {
    autoSel.addEventListener("change", () => {
      const val = parseInt(autoSel.value, 10);
      refreshIntervalMs = (isNaN(val) ? 5 : val) * 1000;
      scheduleAutoRefresh();
    });
  }
  // Actions: describe/logs
  const tableContainer = document.getElementById("table-container");
  if (tableContainer) {
    tableContainer.addEventListener("click", (e) => {
      // toggle expand for name cell
      const nameSpan = e.target && e.target.closest(".name-text");
      if (nameSpan) {
        const td = nameSpan.closest(".cell-name");
        if (td) {
          const expanded = td.classList.toggle("expanded");
          nameSpan.setAttribute("aria-expanded", expanded ? "true" : "false");
          e.stopPropagation();
          return;
        }
      }
      const btnDescribe = e.target && e.target.closest(".btn-describe");
      const btnLogs = e.target && e.target.closest(".btn-logs");
      if (!btnDescribe && !btnLogs) return;
      const tr = e.target.closest("tr[data-key]");
      if (!tr) return;
      const ns = (tr.querySelector(".cell-ns") && tr.querySelector(".cell-ns").textContent) || "";
      const name = (tr.querySelector(".cell-name") && tr.querySelector(".cell-name").textContent) || "";
      const kind = getActiveKind();
      if (btnDescribe) {
        handleDescribe(kind, ns, name);
      } else if (btnLogs) {
        handleLogs(kind, ns, name);
      }
    });
  }
  // Viewer controls
  initViewer();
  // collapse expanded names when clicking outside
  document.addEventListener("click", (e) => {
    if (e.target.closest(".cell-name")) return;
    const expanded = document.querySelectorAll(".cell-name.expanded");
    expanded.forEach((el) => {
      el.classList.remove("expanded");
      const span = el.querySelector(".name-text");
      if (span) span.setAttribute("aria-expanded", "false");
    });
    // clear text selection and blur any focused element inside table
    try {
      const sel = window.getSelection && window.getSelection();
      if (sel && sel.removeAllRanges) sel.removeAllRanges();
    } catch {}
    const table = document.getElementById("table-container");
    if (table && document.activeElement && table.contains(document.activeElement) && document.activeElement.blur) {
      document.activeElement.blur();
    }
  });
  // load initial selection
  const initial = document.querySelector("#resource-list li.active");
  loadKind((initial && initial.dataset.kind) || "pod");
  scheduleAutoRefresh();
});
