const toastRegion = document.querySelector("[data-toast-region]");

function finishToast(toast) {
  toast.addEventListener("animationend", () => toast.remove(), { once: true });
}

function showMemberToast(identifier, role, kind, outcome = "") {
  const toast = document.createElement("div");
  toast.className = "portal-toast border px-4 py-3 shadow-lg";
  toast.dataset.toast = "";
  toast.dataset.toastKind = kind;
  toast.role = kind === "error" ? "alert" : "status";
  const identity = document.createElement("strong");
  identity.className = "font-mono font-semibold";
  identity.textContent = String(identifier).trim().replace(/^@/, "");
  if (kind !== "error") {
    toast.append(identity, ` added as ${role}.`);
  } else if (outcome === "member-not-found") {
    toast.append("Couldn’t add ", identity, ". Please check the username or email and try again.");
  } else if (outcome === "already-member") {
    toast.append(identity, " is already a member of this group.");
  } else {
    toast.append("Couldn’t add ", identity, " to the group. Please try again.");
  }
  toastRegion?.replaceChildren(toast);
  finishToast(toast);
}

function manageMenus() {
  return document.querySelectorAll("[data-manage-menu]");
}
document.addEventListener("toggle", (event) => {
  const menu = event.target.closest("[data-manage-menu]");
  if (menu?.open) {
    for (const other of manageMenus()) if (other !== menu) other.open = false;
  }
}, true);
document.addEventListener("click", (event) => {
  if (!event.target.closest("[data-manage-menu]")) {
    for (const menu of manageMenus()) menu.open = false;
  }
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") for (const menu of manageMenus()) menu.open = false;
});

document.addEventListener("submit", async (event) => {
  const form = event.target.closest("[data-member-picker]");
  if (!form) return;
  event.preventDefault();
  const submit = form.querySelector('button[type="submit"]');
  if (!submit || submit.disabled) return;
  const data = new URLSearchParams(new FormData(form));
  const identifier = data.get("identifier");
  const role = data.get("role") === "admin" ? "Administrator" : "Member";
  submit.disabled = true;
  try {
    const response = await fetch(form.action, {
      method: "POST",
      body: data,
      headers: { "X-Zotero-In-Place": "members" },
    });
    if (response.redirected) throw new Error("member add redirected");
    if (!response.ok) {
      const error = new Error(`member add returned ${response.status}`);
      error.outcome = response.headers.get("X-Zotero-Outcome") ?? await response.text();
      throw error;
    }

    form.reset();
    form.querySelector("[data-member-search]")?.dispatchEvent(new Event("input"));
    const refresh = document.querySelector("[data-members-refresh]");
    if (!refresh) throw new Error("member list refresh control is missing");
    refresh.checked = !refresh.checked;
    refresh.dispatchEvent(new Event("change"));
    showMemberToast(identifier, role, "success");
  } catch (error) {
    showMemberToast(identifier, role, "error", error.outcome?.trim());
  } finally {
    submit.disabled = false;
  }
});
