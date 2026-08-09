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
