const state = {
  summary: {},
  rows: [],
  localOnly: [],
  guideStep: 0,
  filters: {
    search: "",
    status: "all",
    category: "all",
    visibility: "all",
    fork: "all",
    sort: "pushed-desc",
  },
  selectedId: "",
};

const labels = {
  "no-local-copy": "Missing local",
  "no-upstream": "No upstream",
  diverged: "Diverged",
  behind: "Behind",
  ahead: "Ahead",
  synced: "Synced",
  dirty: "Dirty",
  unknown: "Unknown",
};

const categoryLabels = {
  skills: "Skills",
  mcp: "MCP",
  memory: "Memory",
  software: "Software",
  docs: "Docs",
  infra: "Infra",
  data: "Data",
  research: "Research",
  games: "Games",
  other: "Other",
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
  categoryFilters: document.querySelector("#categoryFilters"),
  accountsInput: document.querySelector("#accountsInput"),
  authStatusText: document.querySelector("#authStatusText"),
  authLoginButton: document.querySelector("#authLoginButton"),
  authCheckButton: document.querySelector("#authCheckButton"),
  ghPathInput: document.querySelector("#ghPathInput"),
  visibilityFilters: document.querySelector("#visibilityFilters"),
  forkFilters: document.querySelector("#forkFilters"),
  themeButtons: document.querySelector("#themeButtons"),
  sortSelect: document.querySelector("#sortSelect"),
  scanRootsInput: document.querySelector("#scanRootsInput"),
  fetchToggle: document.querySelector("#fetchToggle"),
  maxDepthInput: document.querySelector("#maxDepthInput"),
  generatedAt: document.querySelector("#generatedAt"),
  accountChips: document.querySelector("#accountChips"),
  errorBanner: document.querySelector("#errorBanner"),
  operationPanel: document.querySelector("#operationPanel"),
  operationLabel: document.querySelector("#operationLabel"),
  operationTitle: document.querySelector("#operationTitle"),
  operationPercent: document.querySelector("#operationPercent"),
  operationBar: document.querySelector("#operationBar"),
  operationDetail: document.querySelector("#operationDetail"),
  operationSteps: document.querySelector("#operationSteps"),
  metricsGrid: document.querySelector("#metricsGrid"),
  matchedCount: document.querySelector("#matchedCount"),
  matchedList: document.querySelector("#matchedList"),
  repoTable: document.querySelector("#repoTable"),
  resultCount: document.querySelector("#resultCount"),
  detailPanel: document.querySelector("#detailPanel"),
  localOnlyCount: document.querySelector("#localOnlyCount"),
  localOnlyList: document.querySelector("#localOnlyList"),
  refreshButton: document.querySelector("#refreshButton"),
  guideButton: document.querySelector("#guideButton"),
  onboardingModal: document.querySelector("#onboardingModal"),
  closeGuideButton: document.querySelector("#closeGuideButton"),
  prevGuideButton: document.querySelector("#prevGuideButton"),
  nextGuideButton: document.querySelector("#nextGuideButton"),
  finishGuideButton: document.querySelector("#finishGuideButton"),
  toast: document.querySelector("#toast"),
};

let toastTimer = 0;
let operationTimer = 0;
let operationProgress = 0;
const guideStorageKey = "repo-atlas-guide-v1";
const ghPathStorageKey = "repo-atlas-gh-path";

const scanSteps = [
  ["Auth", "Checking GitHub CLI authentication"],
  ["Remote", "Loading GitHub repositories"],
  ["Local", "Scanning local Git folders"],
  ["Compare", "Comparing remotes and upstream branches"],
  ["Render", "Updating the atlas"],
];

const loginSteps = [
  ["CLI", "Checking GitHub CLI"],
  ["Browser", "Opening GitHub login in your default browser"],
  ["Callback", "Waiting for GitHub CLI to save credentials"],
  ["Verify", "Verifying authenticated account"],
];

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (char) => {
    const replacements = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#039;" };
    return replacements[char];
  });
}

function formatNumber(value) {
  return new Intl.NumberFormat("en-US").format(value || 0);
}

function formatDate(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return date.toLocaleString("en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDateShort(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return date.toLocaleDateString("en-US", { year: "numeric", month: "2-digit", day: "2-digit" });
}

function baseName(value) {
  const parts = String(value || "").split(/[\\/]/).filter(Boolean);
  return parts.at(-1) || value || "";
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function statusLabel(status) {
  return labels[status] || status || labels.unknown;
}

function statusBadge(status) {
  const clean = status || "unknown";
  return `<span class="badge ${escapeHtml(clean)}">${escapeHtml(statusLabel(clean))}</span>`;
}

function categoryBadge(repo) {
  const category = repo.category || "other";
  const label = repo.categoryLabel || categoryLabels[category] || category;
  return `<span class="badge category-${escapeHtml(category)}">${escapeHtml(label)}</span>`;
}

function visibilityBadge(repo) {
  return `<span class="badge ${repo.visibility}">${repo.isPrivate ? "Private" : "Public"}</span>`;
}

function showToast(message) {
  window.clearTimeout(toastTimer);
  elements.toast.textContent = message;
  elements.toast.classList.add("show");
  toastTimer = window.setTimeout(() => elements.toast.classList.remove("show"), 2600);
}

function showGuide(step = 0) {
  state.guideStep = Math.max(0, Math.min(3, step));
  elements.onboardingModal.hidden = false;
  renderGuide();
}

function hideGuide({ remember = true } = {}) {
  elements.onboardingModal.hidden = true;
  if (remember) {
    window.localStorage.setItem(guideStorageKey, "done");
  }
}

function renderGuide() {
  elements.onboardingModal.querySelectorAll("[data-guide-step]").forEach((slide) => {
    slide.classList.toggle("active", Number(slide.dataset.guideStep) === state.guideStep);
  });
  elements.onboardingModal.querySelectorAll(".onboarding-progress span").forEach((dot, index) => {
    dot.classList.toggle("active", index === state.guideStep);
  });
  elements.prevGuideButton.disabled = state.guideStep === 0;
  elements.nextGuideButton.hidden = state.guideStep === 3;
  elements.finishGuideButton.hidden = state.guideStep !== 3;
}

function showError(message) {
  if (!message) {
    clearError();
    return;
  }
  elements.errorBanner.textContent = message;
  elements.errorBanner.hidden = false;
}

function clearError() {
  elements.errorBanner.textContent = "";
  elements.errorBanner.hidden = true;
}

function getGhPath() {
  return elements.ghPathInput.value.trim();
}

function withGhPath(payload = {}) {
  return { ...payload, ghPath: getGhPath() };
}

function setAuthStatus(status) {
  const installed = Boolean(status && status.installed);
  const authenticated = Boolean(status && status.authenticated);
  elements.authStatusText.className = authenticated ? "ok" : installed ? "warn" : "bad";
  if (!installed) {
    elements.authStatusText.textContent = "GitHub CLI not found";
  } else if (authenticated) {
    elements.authStatusText.textContent = `Authenticated as ${status.login || "GitHub user"}`;
  } else {
    elements.authStatusText.textContent = "GitHub CLI installed, login needed";
  }
}

async function checkAuthStatus({ quiet = false } = {}) {
  if (!quiet) startOperation("GitHub status", "Checking authentication", loginSteps.slice(0, 1));
  try {
    const response = await fetch("/api/auth/status", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(withGhPath()),
    });
    const status = await response.json();
    setAuthStatus(status);
    if (!response.ok || status.ok === false) throw new Error(status.error || "auth status failed");
    if (!quiet) completeOperation("Status checked", status.message || "GitHub CLI status updated");
    return status;
  } catch (error) {
    elements.authStatusText.className = "bad";
    elements.authStatusText.textContent = error.message;
    if (!quiet) failOperation("Status check failed", error.message);
    return { installed: false, authenticated: false, error: error.message };
  }
}

async function loginWithGitHub() {
  startOperation("GitHub login", "Opening browser login", loginSteps);
  clearError();
  elements.authLoginButton.disabled = true;
  elements.authCheckButton.disabled = true;
  try {
    const response = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(withGhPath()),
    });
    const data = await response.json();
    if (!response.ok || !data.ok) throw new Error(data.error || "GitHub login failed");
    setAuthStatus(data.status || {});
    completeOperation("GitHub login complete", data.message || "Credentials saved by GitHub CLI");
  } catch (error) {
    showError(error.message);
    failOperation("GitHub login failed", error.message);
  } finally {
    elements.authLoginButton.disabled = false;
    elements.authCheckButton.disabled = false;
  }
}

function startOperation(label, title, steps) {
  window.clearInterval(operationTimer);
  operationProgress = 4;
  elements.operationPanel.hidden = false;
  elements.operationPanel.classList.remove("complete", "failed");
  elements.operationLabel.textContent = label;
  elements.operationTitle.textContent = title;
  elements.operationDetail.textContent = steps[0]?.[1] || "Starting...";
  elements.operationSteps.innerHTML = steps
    .map((step, index) => `<span class="${index === 0 ? "active" : ""}" data-step-index="${index}">${escapeHtml(step[0])}</span>`)
    .join("");
  setOperationProgress(4, steps);
  operationTimer = window.setInterval(() => {
    const next = Math.min(operationProgress + Math.max(1, Math.round((88 - operationProgress) / 9)), 88);
    setOperationProgress(next, steps);
  }, 520);
}

function setOperationProgress(value, steps) {
  operationProgress = value;
  const percent = Math.max(0, Math.min(100, Math.round(value)));
  elements.operationPercent.textContent = `${percent}%`;
  elements.operationBar.style.width = `${percent}%`;
  if (steps && steps.length) {
    const activeIndex = Math.min(steps.length - 1, Math.floor((percent / 100) * steps.length));
    elements.operationSteps.querySelectorAll("span").forEach((step, index) => {
      step.classList.toggle("active", index <= activeIndex);
    });
    elements.operationDetail.textContent = steps[activeIndex]?.[1] || elements.operationDetail.textContent;
  }
}

function completeOperation(title, detail) {
  window.clearInterval(operationTimer);
  elements.operationPanel.classList.add("complete");
  elements.operationTitle.textContent = title;
  elements.operationDetail.textContent = detail;
  setOperationProgress(100);
  elements.operationSteps.querySelectorAll("span").forEach((step) => step.classList.add("active"));
}

function failOperation(title, detail) {
  window.clearInterval(operationTimer);
  elements.operationPanel.classList.add("failed");
  elements.operationTitle.textContent = title;
  elements.operationDetail.textContent = detail;
  setOperationProgress(Math.max(operationProgress, 12));
}

function parseScanRoots() {
  return elements.scanRootsInput.value
    .split(/\r?\n|;/)
    .map((value) => value.trim())
    .filter(Boolean);
}

function normalizeAccountInput(value) {
  const clean = String(value || "").trim();
  if (!clean) return null;
  const lower = clean.toLowerCase();
  if (["current", "current gh", "current gh login", "default", "active gh"].includes(lower)) return "";
  if (lower === "leave empty for current gh login") return null;
  return clean;
}

function parseAccounts() {
  return elements.accountsInput.value
    .split(/\r?\n|;|,/)
    .map(normalizeAccountInput)
    .filter((value) => value !== null)
    .filter((value, index, values) => values.findIndex((item) => item.toLowerCase() === value.toLowerCase()) === index);
}

function hydrateScanControls() {
  const roots = asArray(state.summary.scanRoots)
    .map(String)
    .filter(Boolean);
  if (roots.length && !elements.scanRootsInput.value.trim()) {
    elements.scanRootsInput.value = roots.join("\n");
  }
  if (typeof state.summary.versionCheckUsedFetch === "boolean") {
    elements.fetchToggle.checked = state.summary.versionCheckUsedFetch;
  }
  const accounts = asArray(state.summary.accounts)
    .map((account) => account.alias || account.login)
    .filter(Boolean);
  const fallbackAccount = state.summary.accountAlias || state.summary.accountLogin || "";
  if (!elements.accountsInput.value.trim()) {
    if (accounts.length) {
      elements.accountsInput.value = accounts.join("\n");
    } else if (fallbackAccount) {
      elements.accountsInput.value = fallbackAccount.split(/\s*,\s*/).filter(Boolean).join("\n");
    }
  }
}

async function loadInventory() {
  elements.ghPathInput.value = window.localStorage.getItem(ghPathStorageKey) || "";
  const response = await fetch("/api/inventory", { cache: "no-store" });
  if (!response.ok) throw new Error(`inventory ${response.status}`);
  const data = await response.json();
  if (data.ok === false && data.error) {
    showError(data.error);
  } else {
    clearError();
  }
  state.summary = data.summary || {};
  state.rows = asArray(data.rows);
  state.localOnly = asArray(data.localOnly);
  if (!state.selectedId) {
    const firstLocal = state.rows.find((repo) => repo.localMatchCount > 0);
    state.selectedId = (firstLocal || state.rows[0] || {}).id || "";
  }
  hydrateScanControls();
  render();
  checkAuthStatus({ quiet: true });
  if (!window.localStorage.getItem(guideStorageKey)) {
    showGuide(0);
  }
  if (shouldAutoRefresh()) {
    window.sessionStorage.setItem("repo-atlas-auto-refresh", "1");
    await refreshInventory({ automatic: true });
  }
}

function shouldAutoRefresh() {
  if (window.sessionStorage.getItem("repo-atlas-auto-refresh")) return false;
  const hasRows = state.rows.length > 0 || state.localOnly.length > 0;
  const hasCounts = (state.summary.remoteCount || 0) > 0 || (state.summary.localRepoCount || 0) > 0;
  return !hasRows && !hasCounts && !state.summary.generatedAt;
}

function render() {
  elements.generatedAt.textContent = state.summary.generatedAt
    ? `Generated ${formatDate(state.summary.generatedAt)}`
    : "Inventory";
  renderAccountChips();
  renderMetrics();
  renderMatched();
  renderRepoTable();
  renderDetails();
  renderLocalOnly();
}

function renderAccountChips() {
  const accounts = asArray(state.summary.accounts);
  const chips = accounts
    .map((account) => {
      const label = account.alias || account.login || "current gh";
      const count = typeof account.repoCount === "number" ? ` - ${formatNumber(account.repoCount)}` : "";
      return `<span>${escapeHtml(label)}${escapeHtml(count)}</span>`;
    })
    .join("");
  const errors = asArray(state.summary.accountErrors)
    .map((item) => `<span class="warning">${escapeHtml(item.alias || "account")} failed</span>`)
    .join("");
  elements.accountChips.innerHTML = chips || errors ? chips + errors : "<span>current gh login</span>";
}

function renderMetrics() {
  const missingCount = Math.max((state.summary.remoteCount || 0) - (state.summary.matchedRemoteCount || 0), 0);
  const accountCount = asArray(state.summary.accounts).length || (state.summary.accountLogin ? 1 : 0);
  const categoryCount = Object.keys(state.summary.categoryCounts || {}).length;
  const metrics = [
    ["Remote repos", state.summary.remoteCount],
    ["Accounts", accountCount],
    ["Categories", categoryCount],
    ["Local Git", state.summary.localRepoCount],
    ["Matched", state.summary.matchedRemoteCount],
    ["Missing", missingCount],
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
  if (!matched.length) {
    elements.matchedList.innerHTML = '<div class="empty-line">No local matches yet.</div>';
    return;
  }

  elements.matchedList.innerHTML = matched
    .map((repo) => {
      const firstPath = asArray(repo.localPaths)[0] || "";
      const statusText = asArray(repo.localStatusList).map(statusLabel).join(" / ");
      return `
        <article class="matched-card">
          <button class="plain-select" type="button" data-select-repo="${escapeHtml(repo.id)}" title="${escapeHtml(repo.name)}">
            <span class="matched-title">${escapeHtml(repo.name)}</span>
            <span class="matched-meta">${escapeHtml(repo.categoryLabel || "Other")} - ${escapeHtml(statusText)} - ${formatNumber(repo.localMatchCount)} path${repo.localMatchCount === 1 ? "" : "s"}</span>
          </button>
          ${
            firstPath
              ? `<button class="icon-button" type="button" data-open-path="${escapeHtml(firstPath)}" title="Open ${escapeHtml(baseName(firstPath))}" aria-label="Open local folder">${icons.folder}</button>`
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
    const localPaths = asArray(repo.localPaths);
    const localMatches = asArray(repo.localMatches);
    const statuses = asArray(repo.localStatusList);
    const searchable = [
      repo.name,
      repo.description,
      repo.owner,
      repo.category,
      repo.categoryLabel,
      repo.language,
      repo.defaultBranch,
      localPaths.join(" "),
      statuses.join(" "),
    ]
      .join(" ")
      .toLowerCase();
    if (query && !searchable.includes(query)) return false;

    if (state.filters.category !== "all" && repo.category !== state.filters.category) return false;
    if (state.filters.status === "local" && repo.localMatchCount === 0) return false;
    if (state.filters.status === "dirty" && !localMatches.some((match) => match.dirty)) return false;
    if (
      !["all", "local", "dirty"].includes(state.filters.status) &&
      repo.localStatus !== state.filters.status &&
      !statuses.includes(state.filters.status)
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
      <span>Repository</span>
      <span>Account</span>
      <span>Category</span>
      <span>Status</span>
      <span>Language</span>
      <span>Pushed</span>
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
          <span class="mono">${escapeHtml(repo.owner || "current")}</span>
          <span>${categoryBadge(repo)}</span>
          <span>${statusBadge(repo.localStatus)}</span>
          <span class="mono">${escapeHtml(repo.language || "none")}</span>
          <span class="mono">${escapeHtml(formatDateShort(repo.pushedAt))}</span>
        </button>
      `,
    )
    .join("");
  elements.repoTable.innerHTML = header + (body || '<div class="detail-empty small-empty">No repositories match the current filters.</div>');
}

function renderDetails() {
  const repo = state.rows.find((item) => item.id === state.selectedId);
  if (!repo) {
    elements.detailPanel.innerHTML = '<div class="detail-empty">Choose a repository</div>';
    return;
  }

  const statuses = asArray(repo.localStatusList);
  const statusBadges = statuses.length ? statuses.map(statusBadge).join(" ") : statusBadge(repo.localStatus);
  const localMatches = asArray(repo.localMatches);
  const localPaths = asArray(repo.localPaths);
  const pathList = localMatches.length
    ? localMatches
        .map((match) => {
          const metrics = [
            typeof match.ahead === "number" ? `<span class="badge">ahead ${match.ahead}</span>` : "",
            typeof match.behind === "number" ? `<span class="badge">behind ${match.behind}</span>` : "",
          ].join("");
          return `
            <article class="path-item">
              <div>
                <span class="path-text" title="${escapeHtml(match.path)}">${escapeHtml(match.path)}</span>
                <div class="path-meta">
                  ${statusBadge(match.status)}
                  ${match.dirty ? statusBadge("dirty") : ""}
                  ${metrics}
                </div>
              </div>
              <button class="icon-button" type="button" data-open-path="${escapeHtml(match.path)}" title="Open local folder" aria-label="Open local folder">
                ${icons.folder}
              </button>
            </article>
          `;
        })
        .join("")
    : '<p class="detail-description">No local checkout matched this remote repository.</p>';

  elements.detailPanel.innerHTML = `
    <div class="detail-title">
      <div>${visibilityBadge(repo)} ${categoryBadge(repo)} ${repo.isFork ? '<span class="badge">Fork</span>' : '<span class="badge">Source</span>'} ${statusBadges}</div>
      <h3>${escapeHtml(repo.name)}</h3>
      <p class="detail-description">${escapeHtml(repo.description || "No description")}</p>
    </div>

    <div class="action-row">
      <a class="action-button" href="${escapeHtml(repo.url)}" target="_blank" rel="noreferrer">${icons.external}<span>GitHub</span></a>
      <button class="action-button" type="button" data-copy="${escapeHtml(repo.cloneUrl)}">${icons.git}<span>Clone URL</span></button>
      ${
        localPaths[0]
          ? `<button class="action-button" type="button" data-open-path="${escapeHtml(localPaths[0])}">${icons.folder}<span>Folder</span></button>`
          : ""
      }
      ${
        localPaths[0]
          ? `<button class="action-button" type="button" data-copy="${escapeHtml(localPaths[0])}">${icons.copy}<span>Path</span></button>`
          : ""
      }
    </div>

    <div class="detail-grid">
      <div class="detail-kv"><span>Account</span><strong>${escapeHtml(repo.owner || "current")}</strong></div>
      <div class="detail-kv"><span>Category</span><strong>${escapeHtml(repo.categoryLabel || "Other")}</strong></div>
      <div class="detail-kv"><span>Language</span><strong>${escapeHtml(repo.language || "none")}</strong></div>
      <div class="detail-kv"><span>Default branch</span><strong>${escapeHtml(repo.defaultBranch || "none")}</strong></div>
      <div class="detail-kv"><span>Last pushed</span><strong>${escapeHtml(formatDate(repo.pushedAt) || "none")}</strong></div>
      <div class="detail-kv"><span>Local paths</span><strong>${formatNumber(repo.localMatchCount)}</strong></div>
    </div>

    <div class="path-list">${pathList}</div>
  `;
}

function renderLocalOnly() {
  elements.localOnlyCount.textContent = formatNumber(state.localOnly.length);
  if (!state.localOnly.length) {
    elements.localOnlyList.innerHTML = '<div class="empty-line">No local-only Git folders were found.</div>';
    return;
  }

  elements.localOnlyList.innerHTML = state.localOnly
    .map((local) => {
      const remoteText = asArray(local.remotes)
        .map((remote) => remote.repoKey || remote.url || remote.name)
        .filter(Boolean)
        .join(", ");
      return `
        <article class="local-row">
          <span>
            <span class="local-title">${escapeHtml(baseName(local.path))}</span>
            <span class="repo-description">${escapeHtml(local.path)}</span>
            <span class="repo-description">${escapeHtml(remoteText || "no remote")}</span>
          </span>
          <span>${categoryBadge(local)}</span>
          <span>${statusBadge(local.dirty ? "dirty" : local.status)}</span>
          <span class="mono">${escapeHtml(local.branch || "none")}</span>
          <button class="icon-button" type="button" data-open-path="${escapeHtml(local.path)}" title="Open local folder" aria-label="Open local folder">${icons.folder}</button>
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
  showToast(`Opened ${baseName(localPath)}`);
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
  showToast("Copied");
}

function setActiveButton(container, attribute, value) {
  container.querySelectorAll("button").forEach((button) => {
    button.classList.toggle("active", button.dataset[attribute] === value);
  });
}

function applyTheme(theme) {
  const safeTheme = ["atlas", "midnight", "paper", "aurora"].includes(theme) ? theme : "atlas";
  document.documentElement.dataset.theme = safeTheme;
  window.localStorage.setItem("repo-atlas-theme", safeTheme);
  setActiveButton(elements.themeButtons, "theme", safeTheme);
}

applyTheme(window.localStorage.getItem("repo-atlas-theme") || "atlas");

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

elements.categoryFilters.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-category]");
  if (!button) return;
  state.filters.category = button.dataset.category;
  setActiveButton(elements.categoryFilters, "category", state.filters.category);
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

elements.themeButtons.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-theme]");
  if (!button) return;
  applyTheme(button.dataset.theme);
});

function buildRefreshPayload() {
  const roots = parseScanRoots();
  const maxDepth = Number.parseInt(elements.maxDepthInput.value, 10);
  const payload = {
    fetch: elements.fetchToggle.checked,
    maxDepth: Number.isFinite(maxDepth) ? maxDepth : 10,
  };
  const accounts = parseAccounts();
  if (accounts.length) {
    payload.accounts = accounts;
  }
  if (roots.length) {
    payload.scanRoots = roots;
  }
  return withGhPath(payload);
}

async function refreshInventory({ automatic = false } = {}) {
  const previousTitle = elements.refreshButton.title;
  const payload = buildRefreshPayload();

  startOperation("Repository scan", automatic ? "Auto-scanning empty inventory" : "Scanning repositories", scanSteps);
  elements.refreshButton.disabled = true;
  elements.refreshButton.title = "Scanning";
  clearError();
  try {
    const response = await fetch("/api/refresh", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const data = await response.json();
    if (!response.ok || !data.ok) throw new Error(data.error || "refresh failed");
    state.summary = data.summary || {};
    state.rows = asArray(data.rows);
    state.localOnly = asArray(data.localOnly);
    const accountErrors = asArray(state.summary.accountErrors);
    if (accountErrors.length) {
      showError(accountErrors.map((item) => `${item.alias || "account"}: ${item.error}`).join(" | "));
    }
    if (!state.rows.some((repo) => repo.id === state.selectedId)) {
      const firstLocal = state.rows.find((repo) => repo.localMatchCount > 0);
      state.selectedId = (firstLocal || state.rows[0] || {}).id || "";
    }
    render();
    completeOperation("Scan complete", `${formatNumber(state.rows.length)} repositories loaded`);
  } catch (error) {
    showError(error.message);
    failOperation("Scan failed", error.message);
  } finally {
    elements.refreshButton.disabled = false;
    elements.refreshButton.title = previousTitle;
  }
}

elements.refreshButton.addEventListener("click", async () => {
  await refreshInventory();
});

elements.authLoginButton.addEventListener("click", async () => {
  await loginWithGitHub();
});

elements.authCheckButton.addEventListener("click", async () => {
  await checkAuthStatus();
});

elements.ghPathInput.addEventListener("change", async () => {
  window.localStorage.setItem(ghPathStorageKey, getGhPath());
  await checkAuthStatus();
});

elements.guideButton.addEventListener("click", () => {
  showGuide(0);
});

elements.closeGuideButton.addEventListener("click", () => {
  hideGuide();
});

elements.prevGuideButton.addEventListener("click", () => {
  state.guideStep = Math.max(0, state.guideStep - 1);
  renderGuide();
});

elements.nextGuideButton.addEventListener("click", () => {
  state.guideStep = Math.min(3, state.guideStep + 1);
  renderGuide();
});

elements.finishGuideButton.addEventListener("click", async () => {
  hideGuide();
  await refreshInventory();
});

elements.onboardingModal.addEventListener("click", (event) => {
  if (event.target === elements.onboardingModal) {
    hideGuide();
  }
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !elements.onboardingModal.hidden) {
    hideGuide();
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
  elements.repoTable.innerHTML = `<div class="detail-empty">Failed to load inventory: ${escapeHtml(error.message)}</div>`;
});
