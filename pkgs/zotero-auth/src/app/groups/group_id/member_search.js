const input = document.querySelector("[data-member-search]");
const results = document.querySelector("[data-member-search-results]");
const openControl = document.querySelector("[data-member-search-open]");
let activeIndex = -1;

function setResultsOpen(open) {
  openControl.checked = open;
  openControl.dispatchEvent(new Event("change"));
  input.setAttribute("aria-expanded", String(open));
}

function options() {
  return [...results.querySelectorAll("[data-member-option]")];
}

function closeResults() {
  setResultsOpen(false);
  input.removeAttribute("aria-activedescendant");
  activeIndex = -1;
}

function activate(index) {
  const items = options();
  if (!items.length) return;
  activeIndex = (index + items.length) % items.length;
  for (const [itemIndex, item] of items.entries()) {
    item.setAttribute("aria-selected", String(itemIndex === activeIndex));
  }
  const active = items[activeIndex];
  input.setAttribute("aria-activedescendant", active.id);
  active.scrollIntoView({ block: "nearest" });
}

function select(option) {
  input.value = option.dataset.username;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  closeResults();
  input.focus();
}

results.addEventListener("click", (event) => {
  const option = event.target.closest("[data-member-option]");
  if (option) select(option);
});

input.addEventListener("keydown", (event) => {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    activate(activeIndex + 1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    activate(activeIndex - 1);
  } else if (event.key === "Enter" && activeIndex >= 0) {
    event.preventDefault();
    select(options()[activeIndex]);
  } else if (event.key === "Escape") {
    closeResults();
  }
});

input.addEventListener("input", () => {
  const query = input.value.trim();
  if (query.length < 3) {
    closeResults();
    return;
  }
  activeIndex = -1;
  setResultsOpen(true);
});

document.addEventListener("click", (event) => {
  if (event.target !== input && !results.contains(event.target)) closeResults();
});
