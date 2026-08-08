const toastRegion = document.querySelector("[data-toast-region]");

function finishToast(toast) {
  toast.addEventListener("animationend", () => toast.remove(), { once: true });
}

function showRoleToast(username, role, kind) {
  const toast = document.createElement("div");
  toast.className = "portal-toast border px-4 py-3 shadow-lg";
  toast.dataset.toast = "";
  toast.dataset.toastKind = kind;
  toast.role = kind === "error" ? "alert" : "status";
  if (kind === "error") toast.append("Couldn’t update ");
  const identity = document.createElement("strong");
  identity.className = "font-mono font-semibold";
  identity.textContent = `@${username}`;
  toast.append(identity, kind === "error"
    ? ` to ${role}. Please try again.`
    : ` is now ${role}.`);
  toastRegion?.replaceChildren(toast);
  finishToast(toast);
}

const manageMenus = [...document.querySelectorAll("[data-manage-menu]")];
for (const menu of manageMenus) {
  menu.addEventListener("toggle", () => {
    if (menu.open) {
      for (const other of manageMenus) if (other !== menu) other.open = false;
    }
  });
}
document.addEventListener("click", (event) => {
  if (!event.target.closest("[data-manage-menu]")) {
    for (const menu of manageMenus) menu.open = false;
  }
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") for (const menu of manageMenus) menu.open = false;
});

for (const form of document.querySelectorAll("[data-role-form]")) {
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const toggle = form.querySelector("[data-role-toggle]");
    if (!toggle || toggle.disabled) return;
    const username = new FormData(form).get("identifier");
    const desiredRole = toggle.value;
    const role = desiredRole === "admin" ? "Administrator" : "Member";
    const data = new FormData(form);
    data.set("role", desiredRole);
    toggle.disabled = true;
    try {
      const response = await fetch(form.action, {
        method: "POST",
        body: data,
        headers: { "X-Zotero-In-Place": "role" },
      });
      if (!response.ok || response.redirected) throw new Error(`role update returned ${response.status}`);
      const isAdministrator = desiredRole === "admin";
      toggle.value = isAdministrator ? "member" : "admin";
      toggle.dataset.currentRole = desiredRole;
      toggle.setAttribute("aria-pressed", String(isAdministrator));
      toggle.setAttribute("aria-label", `Administrator is ${isAdministrator ? "on. Toggle off." : "off. Toggle on."}`);
      toggle.querySelector("[data-role-state]").textContent = isAdministrator ? "On" : "Off";
      form.closest("li")?.querySelector("[data-member-role-label]")?.replaceChildren(role);
      showRoleToast(username, role, "success");
    } catch {
      showRoleToast(username, role, "error");
    } finally {
      toggle.disabled = false;
    }
  });
}

const dialog = document.querySelector("[data-confirm-dialog]");
const message = dialog?.querySelector("[data-confirm-message]");
const cancel = dialog?.querySelector("[data-confirm-cancel]");
const submit = dialog?.querySelector("[data-confirm-submit]");
let pendingForm;

for (const form of document.querySelectorAll("form[data-confirm]")) {
  form.addEventListener("submit", (event) => {
    if (form.dataset.confirmed === "true") {
      delete form.dataset.confirmed;
      return;
    }
    event.preventDefault();
    pendingForm = form;
    message.textContent = form.dataset.confirm;
    submit.textContent = form.dataset.confirmAction ?? "Confirm";
    dialog.showModal();
  });
}

cancel?.addEventListener("click", () => dialog.close());
submit?.addEventListener("click", () => {
  const form = pendingForm;
  pendingForm = undefined;
  dialog.close();
  if (form) {
    form.dataset.confirmed = "true";
    form.requestSubmit();
  }
});

dialog?.addEventListener("close", () => {
  pendingForm = undefined;
});
