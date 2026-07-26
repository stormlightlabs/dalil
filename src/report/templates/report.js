(() => {
  const root = document.documentElement;
  const colorScheme = window.matchMedia("(prefers-color-scheme: dark)");
  const storageKey = "codeplat-report-theme";
  let savedTheme = null;

  try {
    savedTheme = window.localStorage.getItem(storageKey);
  } catch {
    // Storage can be unavailable for local files. The system preference still works.
  }

  let followsSystem = savedTheme !== "light" && savedTheme !== "dark";
  const initialTheme = followsSystem ? (colorScheme.matches ? "dark" : "light") : savedTheme;
  root.dataset.theme = initialTheme;
  root.classList.add("has-js");

  const initializeControls = () => {
    const themeToggle = document.querySelector("#theme-toggle");
    const themeLabel = document.querySelector("#theme-toggle-label");
    const copyButton = document.querySelector("#copy-report");
    const reportJson = document.querySelector("#report-json");

    const updateThemeToggle = () => {
      if (!themeToggle || !themeLabel) {
        return;
      }
      const isDark = root.dataset.theme === "dark";
      const nextTheme = isDark ? "light" : "dark";
      themeToggle.setAttribute("aria-label", `Switch to ${nextTheme} mode`);
      themeLabel.textContent = `${nextTheme[0].toUpperCase()}${nextTheme.slice(1)} mode`;
    };

    if (themeToggle) {
      updateThemeToggle();
      themeToggle.addEventListener("click", () => {
        followsSystem = false;
        root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark";
        try {
          window.localStorage.setItem(storageKey, root.dataset.theme);
        } catch {
          // The selected theme still applies for this page view.
        }
        updateThemeToggle();
      });
    }

    const handleColorSchemeChange = (event) => {
      if (followsSystem) {
        root.dataset.theme = event.matches ? "dark" : "light";
        updateThemeToggle();
      }
    };
    if (typeof colorScheme.addEventListener === "function") {
      colorScheme.addEventListener("change", handleColorSchemeChange);
    } else {
      colorScheme.addListener(handleColorSchemeChange);
    }

    if (copyButton && reportJson) {
      copyButton.addEventListener("click", async () => {
        const originalLabel = copyButton.textContent;
        try {
          await navigator.clipboard.writeText(reportJson.textContent);
          copyButton.textContent = "Copied";
        } catch {
          copyButton.textContent = "Copy failed";
        }
        window.setTimeout(() => {
          copyButton.textContent = originalLabel;
        }, 1800);
      });
    }
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initializeControls, { once: true });
  } else {
    initializeControls();
  }
})();
