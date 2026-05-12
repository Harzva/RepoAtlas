const state = {
  summary: {},
  rows: [],
  localOnly: [],
  filters: {
    search: "",
    status: "all",
    visibility: "all",
    fork: "all",
    sort: "pushed-desc",
  },
  selectedId: "",
};

const labels = {
  "no-local-copy": "缺本地",
  "no-upstream": "无 upstream",
  diverged: "分叉",
  behind: "落后",
  ahead: "超前",
  synced: "同步",
  dirty: "未提交",
  unknown: "未知",
};

const icons = {
  folder:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6.5A2.5 2.5 0 0 1 5.5 4H10l2 2h6.5A2.5 2.5 0 0 1 21 8.5v8A3.5 3.5 0 0 1 17.5 20h-11A3.5 3.5 0 0 1 3 16.5v-10Zm2 2v8A1.5 1.5 0 0 0 6.5 18h11a1.5 1.5 0 0 0 1.5-1.5v-8A.5.5 0 0 0 18.5 8h-7.3l-2-2H5.5a.5.5 0 0 0-.5.5Z"/></svg>',
  copy:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 7a3 3 0 0 1 3-3h6a3 3 0 0 1 3 3v6a3 3 0 0 1-3 3v1a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3v-6a3 3 0 0 1 3-3h1V7Zm3-1a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V7a1 1 0 0 0-1-1h-6Zm-3 4H7a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1v-1h-4a3 3 0 0 1-3-3v-3Z"/></svg>',
  external:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 4h6v6h-2V7.4l-7.3 7.3-1.4-1.4L16.6 6H14V4ZM5 6h6v2H6v10h10v-5h2v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1Z"/></svg>',
  git:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m12 2 10 10-10 10L2 12 12 2Zm0 2.8L4.8 12l7.2 7.2 7.2-7.2L12 4.8ZM8 10.2A1.8 1.8 0 1 1 9.2 8l2.8 2.8 2-2A1.8 1.8 0 1 1 15.2 10l-2 2 2 2a1.8 1.8 0 1 1-1.2 1.2l-2-2-2.8 2.8A1.8 1.8 0 1 1 8 14.8l2.8-2.8L8 9.2Z"/></svg>',
};

const elements = {
  searchInput: document.querySelector("#searchInput"),
  statusFilters: document.querySelector("#statusFilters"),
  visibilityFilters: document.querySelector("#visibilityFilters"),
  forkFilters: document.querySelector("#forkFilters"),
  sortSelect: document.querySelector("#sortSelect"),
  generatedAt: document.querySelector("#generatedAt"),
  metricsGrid: document.querySelector("#metricsGrid"),
  matchedCount: document.querySelector("#matchedCount"),
  matchedList: document.querySelector("#matchedList"),
  repoTable: document.querySelector("#repoTable"),
  resultCount: document.querySelector("#resultCount"),
  detailPanel: document.querySelector("#detailPanel"),
  localOnlyCount: document.querySelector("#localOnlyCount"),
  localOnlyList: document.querySelector("#localOnlyList"),
  refreshButton: document.querySelector("#refreshButton"),
  toast: document.querySelector("#toast"),
};

let toastTimer = 0;

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (char) => {
    const replacements = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#039;" };
    return replacements[char];
  });
}

function formatNumber(value) {
  return new Intl.NumberFormat("zh-CN").format(value || 0);
}

function formatDate(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return date.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function baseName(value) {
  const parts = String(value || "").split(/[\\/]/).filter(Boolean);
  return parts.at(-1) || value || "";
}

function statusLabel(status) {
  return labels[status] || status || labels.unknown;
}

function statusBadge(status) {
  const clean = status || "unknown";
  return `<span class="badge ${escapeHtml(clean)}">${escapeHtml(statusLabel(clean))}</span>`;
}

function visibilityBadge(repo) {
  return `<span class="badge ${repo.visibility}">${repo.isPrivate ? "private" : "public"}</span>`;
}

function showToast(message) {
  window.clearTimeout(toastTimer);
  elements.toast.textContent = message;
  elements.toast.classList.add("show");
  toastTimer = window.setTimeout(() => elements.toast.classList.remove("show"), 2400);
}

async function loadInventory() {
  const response = await fetch("/api/inventory", { cache: "no-store" });
  if (!response.ok) throw new Error(`inventory ${response.status}`);
  const data = await response.json();
  state.summary = data.summary;
  state.rows = data.rows;
  state.localOnly = data.localOnly;
  if (!state.selectedId) {
    const firstLocal = state.rows.find((repo) => repo.localMatchCount > 0);
    state.selectedId = (firstLocal || state.rows[0] || {}).id || "";
  }
  render();
}

function render() {
  elements.generatedAt.textContent = state.summary.generatedAt
    ? `Generated ${formatDate(state.summary.generatedAt)}`
    : "Inventory";
  renderMetrics();
  renderMatched();
  renderRepoTable();
  renderDetails();
  renderLocalOnly();
}

function renderMetrics() {
  const metrics = [
    ["远端仓库", state.summary.remoteCount],
    ["本地 Git", state.summary.localRepoCount],
    ["远端有本地", state.summary.matchedRemoteCount],
    ["缺本地副本", (state.summary.remoteCount || 0) - (state.summary.matchedRemoteCount || 0)],
    ["私有仓库", state.summary.privateCount],
    ["Fork", state.summary.forkCount],
  ];

  elements.metricsGrid.innerHTML = metrics
    .map(
      ([label, value]) => `
        <article class="metric-card">
          <span>${escapeHtml(label)}</span>
          <strong>${formatNumber(value)}</strong>
        </article>
      `,
    )
    .join("");
}

function renderMatched() {
  const matched = state.rows.filter((repo) => repo.localMatchCount > 0);
  elements.matchedCount.textContent = formatNumber(matched.length);
  elements.matchedList.innerHTML = matched
    .map((repo) => {
      const firstPath = repo.localPaths[0] || "";
      const statusText = repo.localStatusList.map(statusLabel).join(" / ");
      return `
        <article class="matched-card">
          <button class="plain-select" type="button" data-select-repo="${escapeHtml(repo.id)}" title="${escapeHtml(repo.name)}">
            <span class="matched-title">${escapeHtml(repo.name)}</span>
            <span class="matched-meta">${escapeHtml(statusText)} · ${formatNumber(repo.localMatchCount)} path</span>
          </button>
          ${
            firstPath
              ? `<button class="icon-button" type="button" data-open-path="${escapeHtml(firstPath)}" title="打开 ${escapeHtml(baseName(firstPath))}" aria-label="打开本地目录">${icons.folder}</button>`
              : ""
          }
        </article>
      `;
    })
    .join("");
}

function filteredRows() {
  const query = state.filters.search.trim().toLowerCase();
  const rows = state.rows.filter((repo) => {
    const searchable = [
      repo.name,
      repo.description,
      repo.language,
      repo.defaultBranch,
      repo.localPaths.join(" "),
      repo.localStatusList.join(" "),
    ]
      .join(" ")
      .toLowerCase();
    if (query && !searchable.includes(query)) return false;

    if (state.filters.status === "local" && repo.localMatchCount === 0) return false;
    if (state.filters.status === "dirty" && !repo.localMatches.some((match) => match.dirty)) return false;
    if (
      !["all", "local", "dirty"].includes(state.filters.status) &&
      repo.localStatus !== state.filters.status &&
      !repo.localStatusList.includes(state.filters.status)
    ) {
      return false;
    }

    if (state.filters.visibility !== "all" && repo.visibility !== state.filters.visibility) return false;
    if (state.filters.fork === "fork" && !repo.isFork) return false;
    if (state.filters.fork === "source" && repo.isFork) return false;
    return true;
  });

  return rows.sort((a, b) => {
    if (state.filters.sort === "name-asc") return a.name.localeCompare(b.name);
    if (state.filters.sort === "status-asc") return a.localStatus.localeCompare(b.localStatus) || a.name.localeCompare(b.name);
    if (state.filters.sort === "language-asc") return (a.language || "zz").localeCompare(b.language || "zz") || a.name.localeCompare(b.name);
    return String(b.pushedAt || "").localeCompare(String(a.pushedAt || ""));
  });
}

function renderRepoTable() {
  const rows = filteredRows();
  elements.resultCount.textContent = formatNumber(rows.length);
  const header = `
    <div class="table-header" role="row">
      <span>仓库</span>
      <span>可见性</span>
      <span>状态</span>
      <span>语言</span>
      <span>默认分支</span>
      <span>推送</span>
    </div>
  `;
  const body = rows
    .map(
      (repo) => `
        <button class="repo-row ${repo.id === state.selectedId ? "active" : ""}" type="button" data-select-repo="${escapeHtml(repo.id)}" role="row">
          <span>
            <span class="repo-name">${escapeHtml(repo.name)}</span>
            <span class="repo-description">${escapeHtml(repo.description || repo.url)}</span>
          </span>
          <span>${visibilityBadge(repo)}</span>
          <span>${statusBadge(repo.localStatus)}</span>
          <span class="mono">${escapeHtml(repo.language || "none")}</span>
          <span class="mono">${escapeHtml(repo.defaultBranch || "none")}</span>
          <span class="mono">${escapeHtml(formatDate(repo.pushedAt).slice(0, 10))}</span>
        </button>
      `,
    )
    .join("");
  elements.repoTable.innerHTML = header + body;
}

function renderDetails() {
  const repo = state.rows.find((item) => item.id === state.selectedId);
  if (!repo) {
    elements.detailPanel.innerHTML = '<div class="detail-empty">未选择仓库</div>';
    return;
  }

  const statusBadges = repo.localStatusList.map(statusBadge).join(" ");
  const pathList = repo.localMatches.length
    ? repo.localMatches
        .map(
          (match) => `
            <article class="path-item">
              <div>
                <span class="path-text" title="${escapeHtml(match.path)}">${escapeHtml(match.path)}</span>
                <div class="path-meta">
                  ${statusBadge(match.status)}
                  ${match.dirty ? statusBadge("dirty") : ""}
                  <span class="badge">ahead ${match.ahead ?? ""}</span>
                  <span class="badge">behind ${match.behind ?? ""}</span>
                </div>
              </div>
              <button class="icon-button" type="button" data-open-path="${escapeHtml(match.path)}" title="打开本地目录" aria-label="打开本地目录">
                ${icons.folder}
              </button>
            </article>
          `,
        )
        .join("")
    : '<p class="detail-description">no-local-copy</p>';

  elements.detailPanel.innerHTML = `
    <div class="detail-title">
      <div>${visibilityBadge(repo)} ${repo.isFork ? '<span class="badge">fork</span>' : '<span class="badge">source</span>'} ${statusBadges}</div>
      <h3>${escapeHtml(repo.name)}</h3>
      <p class="detail-description">${escapeHtml(repo.description || "No description")}</p>
    </div>

    <div class="action-row">
      <a class="action-button" href="${escapeHtml(repo.url)}" target="_blank" rel="noreferrer">${icons.external}<span>GitHub</span></a>
      <button class="action-button" type="button" data-copy="${escapeHtml(repo.cloneUrl)}">${icons.git}<span>Clone</span></button>
      ${
        repo.localPaths[0]
          ? `<button class="action-button" type="button" data-open-path="${escapeHtml(repo.localPaths[0])}">${icons.folder}<span>目录</span></button>`
          : ""
      }
      ${
        repo.localPaths[0]
          ? `<button class="action-button" type="button" data-copy="${escapeHtml(repo.localPaths[0])}">${icons.copy}<span>路径</span></button>`
          : ""
      }
    </div>

    <div class="detail-grid">
      <div class="detail-kv"><span>语言</span><strong>${escapeHtml(repo.language || "none")}</strong></div>
      <div class="detail-kv"><span>默认分支</span><strong>${escapeHtml(repo.defaultBranch || "none")}</strong></div>
      <div class="detail-kv"><span>最后推送</span><strong>${escapeHtml(formatDate(repo.pushedAt) || "none")}</strong></div>
      <div class="detail-kv"><span>本地路径</span><strong>${formatNumber(repo.localMatchCount)}</strong></div>
    </div>

    <div class="path-list">${pathList}</div>
  `;
}

function renderLocalOnly() {
  elements.localOnlyCount.textContent = formatNumber(state.localOnly.length);
  elements.localOnlyList.innerHTML = state.localOnly
    .map((local) => {
      const remoteText = (local.remotes || []).map((remote) => remote.repoKey || remote.url || remote.name).filter(Boolean).join(", ");
      return `
        <article class="local-row">
          <span>
            <span class="local-title">${escapeHtml(baseName(local.path))}</span>
            <span class="repo-description">${escapeHtml(local.path)}</span>
            <span class="repo-description">${escapeHtml(remoteText || "no remote")}</span>
          </span>
          <span>${statusBadge(local.dirty ? "dirty" : local.status)}</span>
          <span class="mono">${escapeHtml(local.branch || "none")}</span>
          <button class="icon-button" type="button" data-open-path="${escapeHtml(local.path)}" title="打开本地目录" aria-label="打开本地目录">${icons.folder}</button>
        </article>
      `;
    })
    .join("");
}

async function openLocalPath(localPath) {
  const response = await fetch("/api/open-local", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path: localPath }),
  });
  const data = await response.json().catch(() => ({}));
  if (!response.ok || !data.ok) throw new Error(data.error || "open failed");
  showToast(`已打开 ${baseName(localPath)}`);
}

async function copyText(text) {
  if (!text) return;
  if (navigator.clipboard && window.isSecureContext) {
    await navigator.clipboard.writeText(text);
  } else {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    textarea.remove();
  }
  showToast("已复制");
}

function setActiveButton(container, attribute, value) {
  container.querySelectorAll("button").forEach((button) => {
    button.classList.toggle("active", button.dataset[attribute] === value);
  });
}

elements.searchInput.addEventListener("input", (event) => {
  state.filters.search = event.target.value;
  renderRepoTable();
});

elements.statusFilters.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-status]");
  if (!button) return;
  state.filters.status = button.dataset.status;
  setActiveButton(elements.statusFilters, "status", state.filters.status);
  renderRepoTable();
});

elements.visibilityFilters.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-visibility]");
  if (!button) return;
  state.filters.visibility = button.dataset.visibility;
  setActiveButton(elements.visibilityFilters, "visibility", state.filters.visibility);
  renderRepoTable();
});

elements.forkFilters.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-fork]");
  if (!button) return;
  state.filters.fork = button.dataset.fork;
  setActiveButton(elements.forkFilters, "fork", state.filters.fork);
  renderRepoTable();
});

elements.sortSelect.addEventListener("change", (event) => {
  state.filters.sort = event.target.value;
  renderRepoTable();
});

elements.refreshButton.addEventListener("click", async () => {
  const previousTitle = elements.refreshButton.title;
  elements.refreshButton.disabled = true;
  elements.refreshButton.title = "Scanning";
  showToast("正在重新扫描 GitHub 与本地仓库");
  try {
    const response = await fetch("/api/refresh", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ account: "Harzva", fetch: true }),
    });
    const data = await response.json();
    if (!response.ok || !data.ok) throw new Error(data.error || "refresh failed");
    state.summary = data.summary;
    state.rows = data.rows;
    state.localOnly = data.localOnly;
    if (!state.rows.some((repo) => repo.id === state.selectedId)) {
      const firstLocal = state.rows.find((repo) => repo.localMatchCount > 0);
      state.selectedId = (firstLocal || state.rows[0] || {}).id || "";
    }
    render();
    showToast("扫描完成");
  } catch (error) {
    showToast(error.message);
  } finally {
    elements.refreshButton.disabled = false;
    elements.refreshButton.title = previousTitle;
  }
});

document.addEventListener("click", async (event) => {
  const selectButton = event.target.closest("[data-select-repo]");
  if (selectButton) {
    state.selectedId = selectButton.dataset.selectRepo;
    renderRepoTable();
    renderDetails();
    return;
  }

  const openButton = event.target.closest("[data-open-path]");
  if (openButton) {
    try {
      await openLocalPath(openButton.dataset.openPath);
    } catch (error) {
      showToast(error.message);
    }
    return;
  }

  const copyButton = event.target.closest("[data-copy]");
  if (copyButton) {
    await copyText(copyButton.dataset.copy);
  }
});

loadInventory().catch((error) => {
  elements.repoTable.innerHTML = `<div class="detail-empty">加载失败：${escapeHtml(error.message)}</div>`;
});
