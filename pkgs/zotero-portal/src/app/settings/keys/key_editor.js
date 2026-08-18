const form = document.querySelector("[data-key-editor]");
const libraryToggle = form?.elements["user-library"];
const perGroupToggle = form?.querySelector("[data-per-group-toggle]");
const perGroupPermissions = form?.querySelector("[data-per-group-permissions]");

function syncPersonalPermissions() {
  const enabled = libraryToggle.checked;
  for (const name of ["user-notes", "user-write"]) {
    const input = form.elements[name];
    input.disabled = !enabled;
    if (!enabled) input.checked = false;
  }
}

function selectedPermission(container) {
  return container.querySelector("input[type=radio]:checked")?.value ?? "none";
}

function groupAccess(permission) {
  if (permission === "write") return { library: true, write: true };
  if (permission === "read") return { library: true, write: false };
  return undefined;
}

function accessFromForm() {
  const groups = {};
  const library = form.elements["user-library"].checked;
  const allGroups = groupAccess(selectedPermission(form.querySelector("[data-group-permission='all']")));
  if (allGroups) groups.all = allGroups;

  if (perGroupToggle.checked) {
    for (const container of form.querySelectorAll("[data-group-permission]:not([data-group-permission='all'])")) {
      const groupID = container.dataset.groupPermission;
      if (!groupID) continue;
      const permission = groupAccess(selectedPermission(container));
      if (permission) groups[groupID] = permission;
    }
  }

  return {
    user: {
      library,
      notes: form.elements["user-notes"].checked,
      files: library,
      write: form.elements["user-write"].checked,
    },
    groups,
  };
}

function syncPermissionSummary() {
  const summary = form?.querySelector("[data-permissions-summary]");
  if (!summary) return;
  const entries = [];
  const access = accessFromForm();
  if (access.user.library) entries.push("Access to read your personal library");
  if (access.user.notes) entries.push("Access to read notes in your personal library");
  if (access.user.write) entries.push("Access to modify your personal library");

  const all = access.groups.all;
  if (all?.write) {
    entries.push("Access to read and modify any of your group libraries");
  } else if (all?.library) {
    entries.push("Access to read any of your group libraries");
  }

  if (perGroupToggle.checked) {
    for (const container of form.querySelectorAll("[data-key-group]")) {
      const permission = selectedPermission(container.querySelector("[data-group-permission]"));
      if (permission === "write") {
        entries.push(`Access to read and modify library for group "${container.dataset.keyGroup}"`);
      } else if (permission === "read") {
        entries.push(`Access to read library for group "${container.dataset.keyGroup}"`);
      }
    }
  }

  summary.replaceChildren(...(entries.length ? entries : ["No additional permissions selected"]).map((entry) => {
    const item = document.createElement("li");
    item.textContent = entry;
    return item;
  }));
}

perGroupToggle?.addEventListener("change", () => {
  perGroupPermissions.hidden = !perGroupToggle.checked;
  syncPermissionSummary();
});

libraryToggle?.addEventListener("change", syncPersonalPermissions);
form?.addEventListener("change", syncPermissionSummary);
if (libraryToggle) syncPersonalPermissions();
syncPermissionSummary();

form?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const actions = document.querySelector("[data-key-editor-actions]");
  const submit = actions.querySelector("[data-save-key]");
  const created = form.querySelector("[data-key-created]");
  const saved = form.querySelector("[data-key-saved]");
  const error = form.querySelector("[data-key-error]");
  const key = form.dataset.key || null;
  const endpoint = key
    ? `${form.action}?${new URLSearchParams({ key })}`
    : form.action;
  submit.disabled = true;
  created.hidden = true;
  saved.hidden = true;
  error.hidden = true;

  if (!form.elements.name.value.trim()) {
    error.textContent = "Key is missing name";
    error.hidden = false;
    submit.disabled = false;
    return;
  }

  try {
    const response = await fetch(endpoint, {
      method: "POST",
      headers: {
        "Accept": "application/json",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        key,
        name: form.elements.name.value.trim(),
        access: accessFromForm(),
      }),
    });
    if (!response.ok) {
      const message = (await response.text()).trim();
      throw new Error(message && !message.startsWith("<") ? message : "Error saving key");
    }
    const result = await response.json();
    if (result.created) {
      form.dataset.key = result.key;
      form.querySelector("[data-key-value]").textContent = result.key;
      form.querySelector("[data-key-editor-fields]").hidden = true;
      actions.hidden = true;
      created.hidden = false;
    } else {
      saved.hidden = false;
    }
  } catch (cause) {
    error.textContent = cause.message || "Error saving key";
    error.hidden = false;
  } finally {
    submit.disabled = false;
  }
});
