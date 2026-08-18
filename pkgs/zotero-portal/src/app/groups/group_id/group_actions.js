function showFeedback(element, message, error = false) {
  if (!element) return;
  element.hidden = false;
  element.textContent = message;
  element.setAttribute("role", error ? "alert" : "status");
  element.classList.toggle("text-red-800", error);
  element.classList.toggle("text-stone-600", !error);
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
      showFeedback(feedback, "Roles Updated");
    } catch (error) {
      for (const select of changed) select.disabled = false;
      showFeedback(feedback, error.message || "Couldn’t update the member roles.", true);
    } finally {
      submit.disabled = false;
    }
    return;
  }

});
