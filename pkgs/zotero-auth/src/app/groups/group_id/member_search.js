const input = document.querySelector("[data-member-search]");
const results = document.querySelector("[data-member-search-results]");
let requestNumber = 0;
let timer;
let activeIndex = -1;

function options() {
  return [...results.querySelectorAll("[data-member-option]")];
}

function closeResults() {
  results.hidden = true;
  input.setAttribute("aria-expanded", "false");
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
  input.value = `@${option.dataset.username}`;
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
  clearTimeout(timer);
  const query = input.value.trim();
  if (query.length < 2) {
    closeResults();
    results.replaceChildren();
    return;
  }
  timer = setTimeout(async () => {
    const currentRequest = ++requestNumber;
    const url = new URL(input.dataset.searchUrl, window.location.origin);
    url.searchParams.set("q", query);
    try {
      const response = await fetch(url, { headers: { Accept: "text/html" } });
      if (!response.ok || currentRequest !== requestNumber) return;
      results.innerHTML = await response.text();
      activeIndex = -1;
      results.hidden = false;
      input.setAttribute("aria-expanded", "true");
    } catch {
      if (currentRequest === requestNumber) closeResults();
    }
  }, 180);
});

document.addEventListener("click", (event) => {
  if (event.target !== input && !results.contains(event.target)) closeResults();
});
