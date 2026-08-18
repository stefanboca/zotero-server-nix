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

perGroupToggle?.addEventListener("change", () => {
  perGroupPermissions.hidden = !perGroupToggle.checked;
});

libraryToggle?.addEventListener("change", syncPersonalPermissions);
if (libraryToggle) syncPersonalPermissions();

form?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const submit = form.querySelector("button[type=submit]");
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
      throw new Error(message && !message.startsWith("<") ? message : "Couldn’t save this API key.");
    }
    const result = await response.json();
    if (result.created) {
      form.dataset.key = result.key;
      form.querySelector("[data-key-value]").textContent = result.key;
      form.querySelector("[data-key-editor-fields]").hidden = true;
      created.hidden = false;
    } else {
      saved.hidden = false;
    }
  } catch (cause) {
    error.textContent = cause.message || "Couldn’t save this API key.";
    error.hidden = false;
  } finally {
    submit.disabled = false;
  }
});
