const state = {
  summary: {},
  rows: [],
  localOnly: [],
  localProjects: [],
  repoDetails: {},
  repoDetailsLoading: {},
  lastAuthStatus: null,
  guideStep: 0,
  filters: {
    search: "",
    status: "all",
    category: "all",
    visibility: "all",
    fork: "all",
    sort: "time-desc",
  },
  selectedId: "",
};

const labels = {
  "no-local-copy": "Missing local",
  "no-upstream": "No upstream",
  "not-git": "No Git",
  diverged: "Diverged",
  behind: "Behind",
  ahead: "Ahead",
  synced: "Synced",
  dirty: "Dirty",
  unknown: "Unknown",
};

const categoryLabels = {
  agents: "Agents",
  memory: "Memory",
  skills: "Skills",
  mcp: "MCP",
  workflow: "Workflow",
  rules: "Rules",
  hook: "Hooks",
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
  authPlusButton: document.querySelector("#authPlusButton"),
  authCheckButton: document.querySelector("#authCheckButton"),
  ghPathInput: document.querySelector("#ghPathInput"),
  loginPlusModal: document.querySelector("#loginPlusModal"),
  closeLoginPlusButton: document.querySelector("#closeLoginPlusButton"),
  loginAccountInput: document.querySelector("#loginAccountInput"),
  addAccountButton: document.querySelector("#addAccountButton"),
  webLoginPlusButton: document.querySelector("#webLoginPlusButton"),
  checkLoginPlusButton: document.querySelector("#checkLoginPlusButton"),
  tokenAccessInput: document.querySelector("#tokenAccessInput"),
  tokenLoginButton: document.querySelector("#tokenLoginButton"),
  authAccountsList: document.querySelector("#authAccountsList"),
  visibilityFilters: document.querySelector("#visibilityFilters"),
  forkFilters: document.querySelector("#forkFilters"),
  themeButtons: document.querySelector("#themeButtons"),
  sortSelect: document.querySelector("#sortSelect"),
  scanRootsInput: document.querySelector("#scanRootsInput"),
  maxDepthInput: document.querySelector("#maxDepthInput"),
  generatedAt: document.querySelector("#generatedAt"),
  accountChips: document.querySelector("#accountChips"),
  errorBanner: document.querySelector("#errorBanner"),
  operationPanel: document.querySelector("#operationPanel"),
  operationLabel: document.querySelector("#operationLabel"),
  operationTitle: document.querySelector("#operationTitle"),
  operationPercent: document.querySelector("#operationPercent"),
  operationLanes: document.querySelectorAll("[data-operation-lane]"),
  scanOperationBar: document.querySelector("#scanOperationBar"),
  scanOperationPercent: document.querySelector("#scanOperationPercent"),
  scanOperationStatus: document.querySelector("#scanOperationStatus"),
  fetchOperationBar: document.querySelector("#fetchOperationBar"),
  fetchOperationPercent: document.querySelector("#fetchOperationPercent"),
  fetchOperationStatus: document.querySelector("#fetchOperationStatus"),
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
  localProjectCount: document.querySelector("#localProjectCount"),
  localProjectList: document.querySelector("#localProjectList"),
  refreshButton: document.querySelector("#refreshButton"),
  fetchButton: document.querySelector("#fetchButton"),
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
let activeOperationLane = "scan";
const guideStorageKey = "repo-atlas-guide-v1";
const ghPathStorageKey = "repo-atlas-gh-path";

const scanSteps = [
  ["Remote", "Loading GitHub repository lists"],
  ["Local", "Indexing local folders in parallel"],
  ["Git", "Reading local Git metadata"],
  ["Compare", "Matching local and remote repositories"],
  ["Render", "Updating the atlas"],
];

const fetchSteps = [
  ["Load", "Reading known local Git repositories"],
  ["Fetch", "Fetching remotes in parallel"],
  ["Compare", "Updating ahead and behind status"],
  ["Render", "Refreshing the atlas"],
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

function timeValue(value) {
  if (!value) return 0;
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? 0 : date.valueOf();
}

function repoActivityTime(repo) {
  return Math.max(timeValue(repo.updatedAt), timeValue(repo.pushedAt));
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

function categoryItems(repo) {
  const categories = asArray(repo.contextKinds).length
    ? asArray(repo.contextKinds)
    : asArray(repo.categories).length
      ? asArray(repo.categories)
      : [repo.category || "other"];
  const labels = asArray(repo.contextLabels).length
    ? asArray(repo.contextLabels)
    : asArray(repo.categoryLabels).length
      ? asArray(repo.categoryLabels)
      : [repo.categoryLabel || categoryLabels[categories[0]] || categories[0]];
  return categories.map((category, index) => ({
    category: category || "other",
    label: labels[index] || categoryLabels[category] || category,
  }));
}

function categoryText(repo) {
  return categoryItems(repo)
    .map((item) => item.label)
    .join(" / ");
}

function categoryBadges(repo) {
  return `<span class="badge-stack">${categoryItems(repo)
    .map((item) => `<span class="badge category-${escapeHtml(item.category)}">${escapeHtml(item.label)}</span>`)
    .join("")}</span>`;
}

function hasCategory(repo, category) {
  return categoryItems(repo).some((item) => item.category === category);
}

function contextEvidenceItems(repo) {
  const direct = asArray(repo.contextEvidence);
  const local = asArray(repo.localContextMatches).flatMap((item) => asArray(item.evidence));
  return Array.from(new Set([...direct, ...local].filter(Boolean)));
}

function knownRepoKeys() {
  return new Set(state.rows.map((repo) => String(repo.repoKey || "").toLowerCase()).filter(Boolean));
}

function isUnlinkedLocalContext(project) {
  const keys = asArray(project.remoteKeys).map((key) => String(key).toLowerCase()).filter(Boolean);
  if (!keys.length) return true;
  const known = knownRepoKeys();
  return !keys.some((key) => known.has(key));
}

function gitScopeLabel(scope) {
  if (scope === "self") return "Git root";
  if (scope === "inside") return "Inside Git";
  return "No Git";
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
  state.lastAuthStatus = status || null;
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
  renderAuthAccounts(status);
}

function renderAuthAccounts(status = state.lastAuthStatus) {
  const accounts = asArray(status?.accounts);
  if (!elements.authAccountsList) return;
  if (!accounts.length) {
    elements.authAccountsList.innerHTML = '<div class="empty-line">No GitHub CLI accounts detected yet.</div>';
    return;
  }
  elements.authAccountsList.innerHTML = accounts
    .map(
      (account) => `
        <article class="auth-account-item">
          <div>
            <strong>${escapeHtml(account.login || "GitHub account")}</strong>
            <span>${escapeHtml(account.state || "unknown")} - ${escapeHtml(account.gitProtocol || "git")} - ${escapeHtml(account.tokenSource || "credential store")}</span>
          </div>
          <button class="action-button" type="button" data-add-account="${escapeHtml(account.login || "")}">${account.active ? "Active" : "Add"}</button>
        </article>
      `,
    )
    .join("");
}

async function checkAuthStatus({ quiet = false } = {}) {
  if (!quiet) startOperation("GitHub status", "Checking authentication", loginSteps.slice(0, 1), { lane: "scan" });
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

async function loginWithGitHub({ force = false } = {}) {
  startOperation("GitHub login", "Opening browser login", loginSteps, { lane: "scan" });
  clearError();
  elements.authLoginButton.disabled = true;
  elements.authCheckButton.disabled = true;
  try {
    const response = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(withGhPath({ force })),
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

async function loginWithTokenAccess() {
  const token = elements.tokenAccessInput.value.trim();
  if (!token) {
    showToast("Paste a token first");
    return;
  }
  startOperation("GitHub access", "Saving token with GitHub CLI", loginSteps, { lane: "scan" });
  clearError();
  elements.tokenLoginButton.disabled = true;
  try {
    const response = await fetch("/api/auth/token-login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(withGhPath({ token })),
    });
    const data = await response.json();
    elements.tokenAccessInput.value = "";
    if (!response.ok || !data.ok) throw new Error(data.error || "GitHub token login failed");
    setAuthStatus(data.status || {});
    completeOperation("GitHub access saved", data.message || "Credentials saved by GitHub CLI");
  } catch (error) {
    showError(error.message);
    failOperation("GitHub access failed", error.message);
  } finally {
    elements.tokenLoginButton.disabled = false;
  }
}

function showLoginPlus() {
  elements.loginPlusModal.hidden = false;
  renderAuthAccounts();
}

function hideLoginPlus() {
  elements.loginPlusModal.hidden = true;
}

function addAccountToScan(account) {
  const clean = String(account || "").trim();
  if (!clean) return;
  const existing = parseAccounts().filter((value) => value !== "");
  if (!existing.some((item) => item.toLowerCase() === clean.toLowerCase())) {
    const currentText = elements.accountsInput.value.trim();
    elements.accountsInput.value = currentText ? `${currentText}\n${clean}` : clean;
  }
  showToast(`Added ${clean}`);
}

function laneElements(lane) {
  if (lane === "fetch") {
    return {
      bar: elements.fetchOperationBar,
      percent: elements.fetchOperationPercent,
      status: elements.fetchOperationStatus,
    };
  }
  return {
    bar: elements.scanOperationBar,
    percent: elements.scanOperationPercent,
    status: elements.scanOperationStatus,
  };
}

function setLaneProgress(lane, value, statusText = "") {
  const percent = Math.max(0, Math.min(100, Math.round(value)));
  const laneEls = laneElements(lane);
  laneEls.percent.textContent = `${percent}%`;
  laneEls.bar.style.width = `${percent}%`;
  if (statusText) {
    laneEls.status.textContent = statusText;
  }
}

function setActiveLane(lane) {
  activeOperationLane = lane || "scan";
  elements.operationLanes.forEach((item) => {
    const isActive = item.dataset.operationLane === activeOperationLane;
    item.classList.toggle("active", isActive);
    if (isActive) item.classList.remove("complete", "failed");
  });
}

function markActiveLane(className, statusText) {
  elements.operationLanes.forEach((item) => {
    if (item.dataset.operationLane === activeOperationLane) {
      item.classList.add(className);
      item.classList.remove(className === "complete" ? "failed" : "complete");
    }
  });
  laneElements(activeOperationLane).status.textContent = statusText;
}

function startOperation(label, title, steps, options = {}) {
  window.clearInterval(operationTimer);
  setActiveLane(options.lane || "scan");
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
    const cap = 96;
    const next = Math.min(operationProgress + Math.max(1, Math.round((cap - operationProgress) / 10)), cap);
    setOperationProgress(next, steps);
    if (next >= cap) {
      laneElements(activeOperationLane).status.textContent = "Waiting";
      elements.operationDetail.textContent = "Waiting for the current operation to finish...";
    }
  }, 520);
}

function setOperationProgress(value, steps) {
  operationProgress = value;
  const percent = Math.max(0, Math.min(100, Math.round(value)));
  elements.operationPercent.textContent = `${percent}%`;
  setLaneProgress(activeOperationLane, percent);
  if (steps && steps.length) {
    const activeIndex = Math.min(steps.length - 1, Math.floor((percent / 100) * steps.length));
    elements.operationSteps.querySelectorAll("span").forEach((step, index) => {
      step.classList.toggle("active", index <= activeIndex);
    });
    elements.operationDetail.textContent = steps[activeIndex]?.[1] || elements.operationDetail.textContent;
    laneElements(activeOperationLane).status.textContent = steps[activeIndex]?.[0] || "Working";
  }
}

function completeOperation(title, detail) {
  window.clearInterval(operationTimer);
  elements.operationPanel.classList.add("complete");
  elements.operationTitle.textContent = title;
  elements.operationDetail.textContent = detail;
  setOperationProgress(100);
  markActiveLane("complete", "Complete");
  elements.operationSteps.querySelectorAll("span").forEach((step) => step.classList.add("active"));
}

function failOperation(title, detail) {
  window.clearInterval(operationTimer);
  elements.operationPanel.classList.add("failed");
  elements.operationTitle.textContent = title;
  elements.operationDetail.textContent = detail;
  setOperationProgress(Math.max(operationProgress, 12));
  markActiveLane("failed", "Failed");
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
  state.localProjects = asArray(data.localProjects);
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
  const hasRows = state.rows.length > 0 || state.localOnly.length > 0 || state.localProjects.length > 0;
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
  renderLocalProjects();
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
  const categoryCount = Object.keys(state.summary.contextKindCounts || state.summary.categoryCounts || {}).length;
  const metrics = [
    ["Remote repos", state.summary.remoteCount],
    ["Accounts", accountCount],
    ["Context tabs", categoryCount],
    ["Local Git", state.summary.localRepoCount],
    ["Unlinked contexts", state.summary.unlinkedContextCount],
    ["No Git contexts", state.summary.localProjectNoGitCount],
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
            <span class="matched-meta">${escapeHtml(categoryText(repo))} - ${escapeHtml(statusText)} - ${formatNumber(repo.localMatchCount)} path${repo.localMatchCount === 1 ? "" : "s"}</span>
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
      categoryText(repo),
      categoryItems(repo)
        .map((item) => item.category)
        .join(" "),
      contextEvidenceItems(repo).join(" "),
      asArray(repo.localContextMatches)
        .map((item) => [item.name, item.path, categoryText(item), asArray(item.remoteKeys).join(" "), asArray(item.evidence).join(" ")].join(" "))
        .join(" "),
      repo.language,
      repo.defaultBranch,
      localPaths.join(" "),
      statuses.join(" "),
    ]
      .join(" ")
      .toLowerCase();
    if (query && !searchable.includes(query)) return false;

    if (state.filters.category !== "all" && !hasCategory(repo, state.filters.category)) return false;
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
    if (state.filters.sort === "time-desc") return repoActivityTime(b) - repoActivityTime(a) || a.name.localeCompare(b.name);
    if (state.filters.sort === "time-asc") return repoActivityTime(a) - repoActivityTime(b) || a.name.localeCompare(b.name);
    if (state.filters.sort === "name-asc") return a.name.localeCompare(b.name);
    if (state.filters.sort === "status-asc") return a.localStatus.localeCompare(b.localStatus) || a.name.localeCompare(b.name);
    if (state.filters.sort === "language-asc") return (a.language || "zz").localeCompare(b.language || "zz") || a.name.localeCompare(b.name);
    if (state.filters.sort === "pushed-asc") return String(a.pushedAt || "").localeCompare(String(b.pushedAt || "")) || a.name.localeCompare(b.name);
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
      <span>Context</span>
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
          <span>${categoryBadges(repo)}</span>
          <span>${statusBadge(repo.localStatus)}</span>
          <span class="mono">${escapeHtml(repo.language || "none")}</span>
          <span class="mono">${escapeHtml(formatDateShort(repo.pushedAt))}</span>
        </button>
      `,
    )
    .join("");
  elements.repoTable.innerHTML = header + (body || '<div class="detail-empty small-empty">No repositories match the current filters.</div>');
}

function repoPath(repo, suffix = "") {
  const base = repo.url || `https://github.com/${repo.name || repo.repoKey || ""}`;
  return `${base.replace(/\/$/, "")}${suffix}`;
}

function detailItems(items, emptyText) {
  const list = asArray(items);
  if (!list.length) return `<p class="detail-description">${escapeHtml(emptyText)}</p>`;
  return `
    <div class="github-item-list">
      ${list
        .map(
          (item) => `
            <a class="github-item" href="${escapeHtml(item.url || "#")}" target="_blank" rel="noreferrer">
              <span>${item.number ? `#${escapeHtml(item.number)} ` : ""}${escapeHtml(item.title || item.name || item.tagName || item.environment || "GitHub item")}</span>
              <small>${escapeHtml(formatDateShort(item.updatedAt || item.publishedAt || item.createdAt) || item.type || item.latestVersion || "")}</small>
            </a>
          `,
        )
        .join("")}
    </div>
  `;
}

function serviceCard(title, body, actionLabel, url) {
  return `
    <article class="service-card">
      <div>
        <strong>${escapeHtml(title)}</strong>
        <p>${escapeHtml(body)}</p>
      </div>
      ${url ? `<a href="${escapeHtml(url)}" target="_blank" rel="noreferrer">${escapeHtml(actionLabel)}</a>` : ""}
    </article>
  `;
}

function renderGitHubPanel(repo) {
  const detail = state.repoDetails[repo.id];
  const loading = state.repoDetailsLoading[repo.id];
  if (loading) {
    return `
      <section class="github-detail-panel">
        <div class="detail-section-head">
          <h4>GitHub live details</h4>
          <span class="badge">Loading</span>
        </div>
        <div class="empty-line">Loading Issues, Pull Requests, Releases, Pages, Deployments, and Packages...</div>
      </section>
    `;
  }
  if (!detail) {
    return `
      <section class="github-detail-panel">
        <div class="detail-section-head">
          <h4>GitHub live details</h4>
          <button class="action-button" type="button" data-refresh-details="${escapeHtml(repo.id)}">Load live details</button>
        </div>
      </section>
    `;
  }
  if (detail.ok === false) {
    return `
      <section class="github-detail-panel">
        <div class="detail-section-head">
          <h4>GitHub live details</h4>
          <button class="action-button" type="button" data-refresh-details="${escapeHtml(repo.id)}">Retry</button>
        </div>
        <p class="detail-description">${escapeHtml(detail.error || "GitHub details failed to load.")}</p>
      </section>
    `;
  }

  const issues = detail.issues || {};
  const pulls = detail.pullRequests || {};
  const releases = detail.releases || {};
  const pages = detail.pages || {};
  const deployments = detail.deployments || {};
  const packages = detail.packages || {};
  const links = detail.links || {};
  const pagesData = pages.data || {};
  const pagesBody = pages.enabled
    ? `${pagesData.status || "published"}${pagesData.html_url ? ` - ${pagesData.html_url}` : ""}`
    : "No GitHub Pages site detected";
  const releaseBody = asArray(releases.items).length
    ? `${asArray(releases.items)[0].tagName || asArray(releases.items)[0].name} published`
    : "No releases published";
  const packageBody = (packages.count || 0) > 0 ? `${formatNumber(packages.count)} packages published` : "No packages published";
  const deploymentBody = asArray(deployments.items).length
    ? `${asArray(deployments.items)[0].environment || "deployment"} ${formatDateShort(asArray(deployments.items)[0].updatedAt || asArray(deployments.items)[0].createdAt)}`
    : "No recent deployments";

  return `
    <section class="github-detail-panel">
      <div class="detail-section-head">
        <h4>GitHub live details</h4>
        <button class="action-button" type="button" data-refresh-details="${escapeHtml(repo.id)}">Refresh</button>
      </div>

      <div class="github-stat-grid">
        <a href="${escapeHtml(links.issues || repoPath(repo, "/issues"))}" target="_blank" rel="noreferrer"><span>Issues</span><strong>${formatNumber(issues.count)}</strong></a>
        <a href="${escapeHtml(links.pullRequests || repoPath(repo, "/pulls"))}" target="_blank" rel="noreferrer"><span>Pull requests</span><strong>${formatNumber(pulls.count)}</strong></a>
        <a href="${escapeHtml(links.releases || repoPath(repo, "/releases"))}" target="_blank" rel="noreferrer"><span>Releases</span><strong>${formatNumber(releases.count)}</strong></a>
        <a href="${escapeHtml(links.packages || repoPath(repo, "/pkgs"))}" target="_blank" rel="noreferrer"><span>Packages</span><strong>${formatNumber(packages.count)}</strong></a>
      </div>

      <div class="github-section">
        <div class="detail-section-head small">
          <h4>Open issues</h4>
          <a href="${escapeHtml(links.issues || repoPath(repo, "/issues"))}" target="_blank" rel="noreferrer">Open</a>
        </div>
        ${detailItems(issues.items, "No open issues found.")}
      </div>

      <div class="github-section">
        <div class="detail-section-head small">
          <h4>Pull requests</h4>
          <a href="${escapeHtml(links.pullRequests || repoPath(repo, "/pulls"))}" target="_blank" rel="noreferrer">Open</a>
        </div>
        ${detailItems(pulls.items, "No open pull requests found.")}
      </div>

      <div class="service-grid">
        ${serviceCard("Releases", releaseBody, asArray(releases.items).length ? "View releases" : "Create new release", asArray(releases.items).length ? links.releases : links.newRelease)}
        ${serviceCard("GitHub Pages", pagesBody, pages.enabled && pagesData.html_url ? "Open site" : "Pages settings", pages.enabled && pagesData.html_url ? pagesData.html_url : links.pagesSettings)}
        ${serviceCard("Deployments", deploymentBody, "View deployments", links.deployments)}
        ${serviceCard("Packages", packageBody, (packages.count || 0) > 0 ? "View packages" : "Publish package", links.packages)}
      </div>
    </section>
  `;
}

async function loadRepoDetails(repo, { force = false } = {}) {
  if (!repo || (!force && (state.repoDetails[repo.id] || state.repoDetailsLoading[repo.id]))) return;
  state.repoDetailsLoading[repo.id] = true;
  renderDetails();
  try {
    const response = await fetch("/api/repo-details", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(
        withGhPath({
          repoKey: repo.repoKey,
          fullName: repo.name,
          account: repo.accountAlias || repo.owner,
        }),
      ),
    });
    const data = await response.json();
    state.repoDetails[repo.id] = response.ok ? data : { ok: false, error: data.error || "GitHub details failed" };
  } catch (error) {
    state.repoDetails[repo.id] = { ok: false, error: error.message };
  } finally {
    state.repoDetailsLoading[repo.id] = false;
    if (state.selectedId === repo.id) renderDetails();
  }
}

function renderContextEvidence(repo) {
  const evidence = contextEvidenceItems(repo);
  if (!evidence.length) return "";
  return `
    <section class="context-evidence-panel">
      <div class="detail-section-head small">
        <h4>Context evidence</h4>
        <span class="counter-pill">${formatNumber(evidence.length)}</span>
      </div>
      <div class="context-evidence-list">
        ${evidence.map((item) => `<span class="badge evidence">${escapeHtml(item)}</span>`).join("")}
      </div>
    </section>
  `;
}

function renderLocalContextMatches(repo) {
  const contexts = asArray(repo.localContextMatches);
  if (!contexts.length) return "";
  return `
    <section class="context-evidence-panel">
      <div class="detail-section-head small">
        <h4>Local context</h4>
        <span class="counter-pill">${formatNumber(contexts.length)}</span>
      </div>
      <div class="path-list">
        ${contexts
          .map((context) => {
            const scope = context.gitScope || (context.isGitRepo ? "self" : "none");
            return `
              <article class="path-item">
                <div>
                  <span class="path-text" title="${escapeHtml(context.path)}">${escapeHtml(context.name || baseName(context.path))}</span>
                  <div class="path-subtext">${escapeHtml(context.path || "")}</div>
                  <div class="path-meta">
                    ${categoryBadges(context)}
                    <span class="badge git-scope-${escapeHtml(scope)}">${escapeHtml(gitScopeLabel(scope))}</span>
                    ${context.dirty ? statusBadge("dirty") : ""}
                  </div>
                </div>
                <button class="icon-button" type="button" data-open-path="${escapeHtml(context.path)}" title="Open local context" aria-label="Open local context">
                  ${icons.folder}
                </button>
              </article>
            `;
          })
          .join("")}
      </div>
    </section>
  `;
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
      <div>${visibilityBadge(repo)} ${categoryBadges(repo)} ${repo.isFork ? '<span class="badge">Fork</span>' : '<span class="badge">Source</span>'} ${statusBadges}</div>
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
      <div class="detail-kv"><span>Context tabs</span><strong>${escapeHtml(categoryText(repo))}</strong></div>
      <div class="detail-kv"><span>Language</span><strong>${escapeHtml(repo.language || "none")}</strong></div>
      <div class="detail-kv"><span>Default branch</span><strong>${escapeHtml(repo.defaultBranch || "none")}</strong></div>
      <div class="detail-kv"><span>Last pushed</span><strong>${escapeHtml(formatDate(repo.pushedAt) || "none")}</strong></div>
      <div class="detail-kv"><span>Local paths</span><strong>${formatNumber(repo.localMatchCount)}</strong></div>
    </div>

    <div class="path-list">${pathList}</div>
    ${renderContextEvidence(repo)}
    ${renderLocalContextMatches(repo)}
    ${renderGitHubPanel(repo)}
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
          <span>${categoryBadges(local)}</span>
          <span>${statusBadge(local.dirty ? "dirty" : local.status)}</span>
          <span class="mono">${escapeHtml(local.branch || "none")}</span>
          <button class="icon-button" type="button" data-open-path="${escapeHtml(local.path)}" title="Open local folder" aria-label="Open local folder">${icons.folder}</button>
        </article>
      `;
    })
    .join("");
}

function filteredLocalProjects() {
  const query = state.filters.search.trim().toLowerCase();
  return state.localProjects
    .filter((project) => {
      if (!isUnlinkedLocalContext(project)) return false;
      const remoteKeys = asArray(project.remoteKeys).join(" ");
      const searchable = [
        project.name,
        project.path,
        remoteKeys,
        project.nearestGitRoot,
        project.gitScope,
        categoryText(project),
        asArray(project.contextKinds).join(" "),
        asArray(project.evidence).join(" "),
      ]
        .join(" ")
        .toLowerCase();
      if (query && !searchable.includes(query)) return false;
      if (state.filters.category !== "all" && !hasCategory(project, state.filters.category)) return false;
      return true;
    })
    .sort((a, b) => timeValue(b.modifiedAt) - timeValue(a.modifiedAt) || String(a.name || "").localeCompare(String(b.name || "")));
}

function renderLocalProjects() {
  const projects = filteredLocalProjects();
  elements.localProjectCount.textContent = formatNumber(projects.length);
  if (!projects.length) {
    elements.localProjectList.innerHTML = '<div class="empty-line">No unlinked local contexts match the current filters.</div>';
    return;
  }

  elements.localProjectList.innerHTML = projects
    .map((project) => {
      const remoteKeys = asArray(project.remoteKeys).filter(Boolean);
      const evidence = asArray(project.evidence).filter(Boolean).slice(0, 4);
      const scope = project.gitScope || (project.isGitRepo ? "self" : "none");
      const gitBadge = `<span class="badge git-scope-${escapeHtml(scope)}">${escapeHtml(gitScopeLabel(scope))}</span>${
        scope !== "none" ? (project.dirty ? statusBadge("dirty") : statusBadge(project.gitStatus || "unknown")) : ""
      }`;
      return `
        <article class="context-project-card">
          <div class="context-project-title">
            <div>
              <strong>${escapeHtml(project.name || baseName(project.path))}</strong>
              <span>${escapeHtml(project.path)}</span>
            </div>
            <button class="icon-button" type="button" data-open-path="${escapeHtml(project.path)}" title="Open local project" aria-label="Open local project">${icons.folder}</button>
          </div>
          <div class="context-project-meta">
            ${categoryBadges(project)}
            ${gitBadge}
            ${project.branch ? `<span class="badge">${escapeHtml(project.branch)}</span>` : ""}
          </div>
          ${
            evidence.length
              ? `<div class="context-evidence-list compact">${evidence.map((item) => `<span class="badge evidence">${escapeHtml(item)}</span>`).join("")}</div>`
              : ""
          }
          <div class="context-project-foot">
            <span>${escapeHtml(remoteKeys.length ? remoteKeys.join(", ") : "No GitHub remote linked")}</span>
            <span>${escapeHtml(formatDateShort(project.modifiedAt) || "no modified time")}</span>
          </div>
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
  renderLocalProjects();
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
  renderLocalProjects();
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
  renderLocalProjects();
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

function applyInventoryData(data) {
  state.summary = data.summary || {};
  state.rows = asArray(data.rows);
  state.localOnly = asArray(data.localOnly);
  state.localProjects = asArray(data.localProjects);
  const accountErrors = asArray(state.summary.accountErrors);
  if (accountErrors.length) {
    showError(accountErrors.map((item) => `${item.alias || "account"}: ${item.error}`).join(" | "));
  }
  if (!state.rows.some((repo) => repo.id === state.selectedId)) {
    const firstLocal = state.rows.find((repo) => repo.localMatchCount > 0);
    state.selectedId = (firstLocal || state.rows[0] || {}).id || "";
  }
  render();
}

async function refreshInventory({ automatic = false } = {}) {
  const previousTitle = elements.refreshButton.title;
  const payload = buildRefreshPayload();

  startOperation("Repository scan", automatic ? "Auto-scanning empty inventory" : "Scanning repositories", scanSteps, { lane: "scan" });
  elements.refreshButton.disabled = true;
  elements.fetchButton.disabled = true;
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
    applyInventoryData(data);
    completeOperation(
      "Scan complete",
      `${formatNumber(state.rows.length)} repositories and ${formatNumber(state.summary.unlinkedContextCount || filteredLocalProjects().length)} unlinked local contexts loaded`,
    );
  } catch (error) {
    showError(error.message);
    failOperation("Scan failed", error.message);
  } finally {
    elements.refreshButton.disabled = false;
    elements.fetchButton.disabled = false;
    elements.refreshButton.title = previousTitle;
  }
}

async function fetchRemotes() {
  const previousTitle = elements.fetchButton.title;
  startOperation("Git remote fetch", "Fetching known local remotes", fetchSteps, { lane: "fetch" });
  elements.refreshButton.disabled = true;
  elements.fetchButton.disabled = true;
  elements.fetchButton.title = "Fetching remotes";
  clearError();
  try {
    const response = await fetch("/api/fetch-remotes", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: "{}",
    });
    const data = await response.json();
    if (!response.ok || !data.ok) throw new Error(data.error || "fetch remotes failed");
    applyInventoryData(data);
    completeOperation(
      "Fetch complete",
      `${formatNumber(state.summary.localRepoCount || 0)} local Git repositories refreshed`,
    );
  } catch (error) {
    showError(error.message);
    failOperation("Fetch failed", error.message);
  } finally {
    elements.refreshButton.disabled = false;
    elements.fetchButton.disabled = false;
    elements.fetchButton.title = previousTitle;
  }
}

elements.refreshButton.addEventListener("click", async () => {
  await refreshInventory();
});

elements.fetchButton.addEventListener("click", async () => {
  await fetchRemotes();
});

elements.authLoginButton.addEventListener("click", async () => {
  await loginWithGitHub();
});

elements.authPlusButton.addEventListener("click", () => {
  showLoginPlus();
});

elements.authCheckButton.addEventListener("click", async () => {
  await checkAuthStatus();
});

elements.closeLoginPlusButton.addEventListener("click", () => {
  hideLoginPlus();
});

elements.addAccountButton.addEventListener("click", () => {
  addAccountToScan(elements.loginAccountInput.value);
  elements.loginAccountInput.value = "";
});

elements.webLoginPlusButton.addEventListener("click", async () => {
  await loginWithGitHub({ force: true });
});

elements.checkLoginPlusButton.addEventListener("click", async () => {
  await checkAuthStatus();
});

elements.tokenLoginButton.addEventListener("click", async () => {
  await loginWithTokenAccess();
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
  if (event.key === "Escape" && !elements.loginPlusModal.hidden) {
    hideLoginPlus();
  }
  if (event.key === "Escape" && !elements.onboardingModal.hidden) {
    hideGuide();
  }
});

document.addEventListener("click", async (event) => {
  if (event.target === elements.loginPlusModal) {
    hideLoginPlus();
    return;
  }

  const addAccountButton = event.target.closest("[data-add-account]");
  if (addAccountButton) {
    addAccountToScan(addAccountButton.dataset.addAccount);
    return;
  }

  const refreshDetailsButton = event.target.closest("[data-refresh-details]");
  if (refreshDetailsButton) {
    const repo = state.rows.find((item) => item.id === refreshDetailsButton.dataset.refreshDetails);
    await loadRepoDetails(repo, { force: true });
    return;
  }

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
