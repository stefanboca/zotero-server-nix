const dialog = document.querySelector("[data-confirm-dialog]");
const title = dialog?.querySelector("[data-confirm-title]");
const message = dialog?.querySelector("[data-confirm-message]");
const cancel = dialog?.querySelector("[data-confirm-cancel]");
const submit = dialog?.querySelector("[data-confirm-submit]");
const inputContainer = dialog?.querySelector("[data-confirm-input-container]");
const inputLabel = dialog?.querySelector("[data-confirm-input-label]");
const input = dialog?.querySelector("[data-confirm-input]");
const inputHelp = dialog?.querySelector("[data-confirm-input-help]");
const toastRegion = document.querySelector("[data-toast-region]");
let pendingForm;

function finishToast(toast) {
  toast.addEventListener("animationend", () => {
    if (typeof toast.hidePopover === "function" && toast.matches(":popover-open")) {
      toast.hidePopover();
    }
    toast.remove();
  }, { once: true });
}

function showToast(form, kind) {
  const toast = document.createElement("div");
  toast.className = "portal-toast border px-4 py-3 shadow-lg";
  toast.dataset.toast = "";
  toast.dataset.toastKind = kind;
  toast.role = kind === "error" ? "alert" : "status";
  const prefix = form.dataset[`confirm${kind === "error" ? "Error" : "Success"}Prefix`];
  const username = form.dataset[`confirm${kind === "error" ? "Error" : "Success"}Username`];
  const suffix = form.dataset[`confirm${kind === "error" ? "Error" : "Success"}Suffix`];
  const message = form.dataset[`confirm${kind === "error" ? "Error" : "Success"}`];
  if (message) {
    toast.append(message);
  } else {
    if (prefix) toast.append(prefix);
    if (username) {
      const identity = document.createElement("strong");
      identity.className = "font-mono font-semibold";
      identity.textContent = username;
      toast.append(identity);
    }
    if (suffix) toast.append(suffix);
  }
  toastRegion?.replaceChildren(toast);
  if (typeof toast.showPopover === "function") {
    toast.popover = "manual";
    toast.showPopover();
  }
  finishToast(toast);
}

function updateInputState() {
  if (!pendingForm || inputContainer.hidden) {
    submit.disabled = false;
    return;
  }
  const match = pendingForm.dataset.confirmInputMatch;
  submit.disabled = match !== undefined
    ? input.value !== match
    : pendingForm.dataset.confirmInputRequired === "true" && input.value.trim() === "";
}

document.addEventListener("submit", (event) => {
  const form = event.target.closest("form[data-confirm]");
  if (!form) return;
  event.preventDefault();
  if (!dialog) return;
  pendingForm = form;
  title.textContent = form.dataset.confirmTitle ?? "Confirm action";
  message.textContent = form.dataset.confirm;
  submit.textContent = form.dataset.confirmAction ?? "Confirm";
  const needsInput = Boolean(form.dataset.confirmInputName);
  inputContainer.hidden = !needsInput;
  input.value = form.dataset.confirmInputValue ?? "";
  if (needsInput) {
    inputLabel.textContent = form.dataset.confirmInputLabel ?? "Confirmation";
    input.maxLength = Number(form.dataset.confirmInputMaxlength || 524288);
    const match = form.dataset.confirmInputMatch;
    inputHelp.textContent = match === undefined ? "" : `Type “${match}” to confirm.`;
    inputHelp.hidden = match === undefined;
  }
  updateInputState();
  dialog.showModal();
  if (needsInput) input.focus();
});

input?.addEventListener("input", updateInputState);
cancel?.addEventListener("click", () => dialog.close());
dialog?.addEventListener("click", (event) => {
  if (event.target === dialog) dialog.close();
});
submit?.addEventListener("click", async () => {
  const form = pendingForm;
  if (!form || submit.disabled) return;
  submit.disabled = true;
  const data = new URLSearchParams(new FormData(form));
  if (form.dataset.confirmInputName) {
    data.set(form.dataset.confirmInputName, input.value);
  }
  try {
    const response = await fetch(form.action, {
      method: form.method || "POST",
      body: data,
      headers: { "X-Zotero-Confirmed": "true" },
    });
    if (!response.ok || response.redirected) throw new Error(`confirmation returned ${response.status}`);
    const redirect = response.headers.get("location");
    if (redirect && form.dataset.confirmStay !== "true") {
      window.location.assign(redirect);
      return;
    }
    showToast(form, "success");
    if (form.dataset.confirmRemove === "closest") {
      form.closest("[data-confirm-item]")?.remove();
    }
    if (form.dataset.confirmRemoveAll) {
      for (const element of document.querySelectorAll(form.dataset.confirmRemoveAll)) element.remove();
    }
    if (form.dataset.confirmRefresh) {
      const refresh = document.querySelector(form.dataset.confirmRefresh);
      if (refresh) {
        refresh.checked = !refresh.checked;
        refresh.dispatchEvent(new Event("change"));
      }
    }
    dialog.close();
  } catch {
    showToast(form, "error");
    dialog.close();
  } finally {
    submit.disabled = false;
  }
});

dialog?.addEventListener("close", () => {
  pendingForm = undefined;
  input.value = "";
  inputContainer.hidden = true;
});

const automaticForm = document.querySelector("form[data-confirm-auto='true']");
if (automaticForm) {
  automaticForm.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  const url = new URL(window.location.href);
  url.searchParams.delete("dialog");
  window.history.replaceState({}, "", `${url.pathname}${url.search}${url.hash}`);
}
