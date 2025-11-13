let currentViewerMode = "describe"; // "describe" | "logs"

export function openViewer(title, content, mode = "describe") {
  const overlay = document.getElementById("viewer-overlay");
  const titleEl = document.getElementById("viewer-title");
  const yamlEl = document.getElementById("viewer-yaml");
  const treeEl = document.getElementById("viewer-tree");
  const logsEl = document.getElementById("viewer-logs");
  const tabsEl = document.querySelector(".viewer-tabs");
  if (!overlay || !titleEl || !yamlEl || !treeEl || !logsEl) return;
  titleEl.textContent = title || "Viewer";
  currentViewerMode = mode;
  if (mode === "logs") {
    if (tabsEl) tabsEl.style.display = "none";
    if (yamlEl) yamlEl.hidden = true;
    if (treeEl) treeEl.hidden = true;
    if (logsEl) {
      logsEl.hidden = false;
      logsEl.textContent = content || "";
    }
  } else {
    if (tabsEl) tabsEl.style.display = "flex";
    yamlEl.textContent = content || "";
    treeEl.innerHTML = "";
    setActiveViewerTab("yaml");
    if (logsEl) logsEl.hidden = true;
  }
  overlay.classList.add("on");
}

export function closeViewer() {
  const overlay = document.getElementById("viewer-overlay");
  if (overlay) overlay.classList.remove("on");
}

export function setActiveViewerTab(tab) {
  const yamlEl = document.getElementById("viewer-yaml");
  const treeEl = document.getElementById("viewer-tree");
  const tabs = document.querySelectorAll(".viewer-tab");
  tabs.forEach((t) => {
    const isActive = t.getAttribute("data-tab") === tab;
    t.setAttribute("aria-selected", isActive ? "true" : "false");
  });
  if (tab === "tree") {
    if (yamlEl) yamlEl.hidden = true;
    if (treeEl) treeEl.hidden = false;
  } else {
    if (yamlEl) yamlEl.hidden = false;
    if (treeEl) treeEl.hidden = true;
  }
}

function buildTreeNode(key, value) {
  const container = document.createElement("div");
  if (value !== null && typeof value === "object") {
    const details = document.createElement("details");
    const summary = document.createElement("summary");
    summary.innerHTML = `<span class="kv">${key}</span>`;
    details.appendChild(summary);
    if (Array.isArray(value)) {
      value.forEach((v, idx) => {
        const child = buildTreeNode(String(idx), v);
        details.appendChild(child);
      });
    } else {
      Object.keys(value).forEach((k) => {
        const child = buildTreeNode(k, value[k]);
        details.appendChild(child);
      });
    }
    container.appendChild(details);
  } else {
    const line = document.createElement("div");
    const scalar = (value === null) ? "null" : String(value);
    line.innerHTML = `<span class="kv">${key}:</span> <span class="scalar">${scalar}</span>`;
    container.appendChild(line);
  }
  return container;
}

export function renderJsonTree(jsonText) {
  let data = null;
  try {
    data = JSON.parse(jsonText);
  } catch {
    return;
  }
  const treeEl = document.getElementById("viewer-tree");
  if (!treeEl) return;
  treeEl.innerHTML = "";
  const root = buildTreeNode("(root)", data);
  treeEl.appendChild(root);
  const firstDetails = treeEl.querySelector("details");
  if (firstDetails) firstDetails.setAttribute("open", "open");
}

export function initViewer() {
  const btnClose = document.getElementById("viewer-close");
  if (btnClose) {
    btnClose.addEventListener("click", closeViewer);
  }
  const btnCopy = document.getElementById("viewer-copy");
  if (btnCopy) {
    btnCopy.addEventListener("click", async () => {
      try {
        let text = "";
        if (currentViewerMode === "logs") {
          const logsEl = document.getElementById("viewer-logs");
          text = (logsEl && logsEl.textContent) || "";
        } else {
          const yamlEl = document.getElementById("viewer-yaml");
          const treeEl = document.getElementById("viewer-tree");
          const activeTab = document.querySelector('.viewer-tab[aria-selected="true"]')?.getAttribute("data-tab") || "yaml";
          text = activeTab === "yaml"
            ? ((yamlEl && yamlEl.textContent) || "")
            : ((treeEl && treeEl.textContent) || "");
        }
        await navigator.clipboard.writeText(text);
      } catch {}
    });
  }
  const tabs = document.querySelectorAll(".viewer-tab");
  tabs.forEach((tabBtn) => {
    tabBtn.addEventListener("click", () => {
      const tab = tabBtn.getAttribute("data-tab");
      setActiveViewerTab(tab);
    });
  });
  const overlay = document.getElementById("viewer-overlay");
  if (overlay) {
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) {
        closeViewer();
      }
    });
  }
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeViewer();
  });
}


