function refreshMembers() {
  const control = document.querySelector("[data-members-refresh]");
  if (!control) return;
  control.checked = !control.checked;
  control.dispatchEvent(new Event("change"));
}

function showFeedback(element, message, error = false) {
  if (!element) return;
  element.hidden = false;
  element.textContent = message;
  element.setAttribute("role", error ? "alert" : "status");
  element.classList.toggle("text-red-800", error);
  element.classList.toggle("dark:text-red-400", error);
  element.classList.toggle("text-stone-600", !error);
  element.classList.toggle("dark:text-stone-400", !error);
}

async function responseError(response, fallback) {
  const message = (await response.text()).trim();
  return message && !message.startsWith("<") ? message : fallback;
}

document.addEventListener("submit", async (event) => {
  const rolesForm = event.target.closest("[data-member-roles]");
  if (rolesForm) {
    event.preventDefault();
    const changed = [...rolesForm.querySelectorAll("[data-member-role]")]
      .filter((select) => select.value !== select.dataset.currentRole);
    const submit = rolesForm.querySelector("button[type=submit]");
    const feedback = rolesForm.querySelector("[data-role-feedback]");
    if (!changed.length) {
      showFeedback(feedback, "No role changes to save.");
      return;
    }

    submit.disabled = true;
    for (const select of changed) select.disabled = true;
    try {
      const response = await fetch(rolesForm.action, {
        method: "PATCH",
        headers: {
          "Accept": "application/json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          members: changed.map((select) => ({
            userId: Number(select.dataset.userId),
            role: select.value,
          })),
        }),
      });
      if (!response.ok) {
        throw new Error(await responseError(response, "Couldn’t update the member roles."));
      }

      const selfUpdate = changed.find((select) => select.dataset.self === "true");
      if (selfUpdate?.value === "remove") {
        window.location.assign(rolesForm.dataset.removedRedirect);
        return;
      }
      if (selfUpdate?.value === "member") {
        window.location.assign(rolesForm.dataset.redirect);
        return;
      }
      for (const select of changed) {
        if (select.value === "remove") {
          select.closest("li").remove();
        } else {
          select.dataset.currentRole = select.value;
          select.disabled = false;
        }
      }
      showFeedback(feedback, "Member roles updated.");
    } catch (error) {
      for (const select of changed) select.disabled = false;
      showFeedback(feedback, error.message || "Couldn’t update the member roles.", true);
    } finally {
      submit.disabled = false;
    }
    return;
  }

  const form = event.target.closest("[data-member-create]");
  if (!form) return;
  event.preventDefault();
  const identifier = form.elements.identifier.value.trim();
  const role = form.elements.role.value;
  const submit = form.querySelector("button[type=submit]");
  const feedback = form.parentElement.querySelector("[data-member-feedback]");
  submit.disabled = true;
  try {
    const response = await fetch(form.action, {
      method: "POST",
      headers: {
        "Accept": "application/json",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ identifier, role }),
    });
    if (!response.ok) {
      throw new Error(await responseError(response, "Couldn’t add that account to the group."));
    }
    const member = await response.json();
    form.elements.identifier.value = "";
    form.elements.identifier.dispatchEvent(new Event("input", { bubbles: true }));
    showFeedback(
      feedback,
      `${member.username} was added as ${role === "admin" ? "an administrator" : "a member"}.`,
    );
    refreshMembers();
  } catch (error) {
    showFeedback(feedback, error.message || "Couldn’t add that account to the group.", true);
  } finally {
    submit.disabled = false;
  }
});
