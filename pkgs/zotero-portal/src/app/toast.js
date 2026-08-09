const toast = document.querySelector("[data-toast]");
if (toast) {
  toast.addEventListener("animationend", () => toast.remove(), { once: true });
  const url = new URL(window.location.href);
  for (const parameter of ["notice", "member", "role", "name"]) {
    url.searchParams.delete(parameter);
  }
  window.history.replaceState({}, "", `${url.pathname}${url.search}${url.hash}`);
}
