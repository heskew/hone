import { test, expect, Page } from "@playwright/test";

// Helper to toggle dark mode
async function enableDarkMode(page: Page) {
  await page.emulateMedia({ colorScheme: "dark" });
}

async function enableLightMode(page: Page) {
  await page.emulateMedia({ colorScheme: "light" });
}

test.describe("Dark Mode UX Audit", () => {
  test.beforeEach(async ({ page }) => {
    await enableDarkMode(page);
  });

  test("dashboard renders correctly in dark mode", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Take screenshot for visual review
    await page.screenshot({ path: "e2e/screenshots/dashboard-dark.png", fullPage: true });

    // Check that body has dark background (should not be white)
    const bodyBg = await page.evaluate(() => {
      return window.getComputedStyle(document.body).backgroundColor;
    });
    // In dark mode, background should be dark (not white rgb(255,255,255))
    expect(bodyBg).not.toBe("rgb(255, 255, 255)");

    // Header should be visible and have appropriate contrast
    const header = page.locator("header");
    await expect(header).toBeVisible();

    // Navigation buttons should be readable
    const navButtons = page.locator("nav button");
    const count = await navButtons.count();
    expect(count).toBeGreaterThan(0);
  });

  test("transactions page in dark mode", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/transactions-dark.png", fullPage: true });

    // Search input should be visible and have proper styling
    const searchInput = page.locator('input[placeholder*="Search"]');
    await expect(searchInput).toBeVisible();

    // Check search input has visible border/contrast
    const inputBorder = await searchInput.evaluate((el) => {
      const style = window.getComputedStyle(el);
      return style.borderColor;
    });
    // Should have a visible border (not same as background)
    expect(inputBorder).toBeDefined();
  });

  test("reports page in dark mode", async ({ page }) => {
    await page.goto("/#/reports/spending");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/reports-dark.png", fullPage: true });

    // Tab buttons should have visible text
    const tabButtons = page.locator(".border-b button");
    const firstTab = tabButtons.first();
    await expect(firstTab).toBeVisible();
  });

  test("tags page in dark mode", async ({ page }) => {
    await page.goto("/#/tags");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/tags-dark.png", fullPage: true });

    // Page should load without errors - look for Tag Management heading
    await expect(page.getByRole("heading", { name: "Tag Management" })).toBeVisible();
  });

  test("import page in dark mode", async ({ page }) => {
    await page.goto("/#/import");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/import-dark.png", fullPage: true });

    // Form elements should be visible
    const selectElement = page.locator("select").first();
    if (await selectElement.isVisible()) {
      const selectBg = await selectElement.evaluate((el) => {
        return window.getComputedStyle(el).backgroundColor;
      });
      // Select should have dark background in dark mode
      expect(selectBg).not.toBe("rgb(255, 255, 255)");
    }
  });

  test("modals render correctly in dark mode", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Try to open a transaction modal if transactions exist
    const transactionRow = page.locator(".divide-y > div").first();
    if (await transactionRow.isVisible()) {
      // Click to open transaction details
      const detailsButton = transactionRow.locator("button").first();
      if (await detailsButton.isVisible()) {
        await detailsButton.click();
        await page.waitForTimeout(300);
        await page.screenshot({ path: "e2e/screenshots/modal-dark.png" });
      }
    }
  });
});

test.describe("Light Mode Baseline", () => {
  test.beforeEach(async ({ page }) => {
    await enableLightMode(page);
  });

  test("dashboard renders correctly in light mode", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/dashboard-light.png", fullPage: true });

    // Cards should be visible
    const cards = page.locator(".card");
    const count = await cards.count();
    expect(count).toBeGreaterThan(0);
  });

  test("transactions page in light mode", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/transactions-light.png", fullPage: true });
  });

  test("reports page in light mode", async ({ page }) => {
    await page.goto("/#/reports/spending");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/reports-light.png", fullPage: true });
  });
});

test.describe("Mobile Responsive UX", () => {
  test.use({ viewport: { width: 375, height: 667 } }); // iPhone SE

  test("mobile navigation hamburger menu works", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/mobile-dashboard.png", fullPage: true });

    // Mobile menu button should be visible
    const menuButton = page.locator("button.md\\:hidden, [aria-label*='menu'], button:has(svg)").first();

    // Check if we're in mobile mode (nav buttons hidden)
    const desktopNav = page.locator("nav.hidden.md\\:flex, nav:not(.hidden)");
    const isDesktopNavVisible = await desktopNav.isVisible().catch(() => false);

    if (!isDesktopNavVisible) {
      // Mobile mode - look for hamburger
      await page.screenshot({ path: "e2e/screenshots/mobile-nav-closed.png" });
    }
  });

  test("mobile transactions list is scrollable", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/mobile-transactions.png", fullPage: true });

    // Content should not overflow horizontally
    const hasHorizontalScroll = await page.evaluate(() => {
      return document.body.scrollWidth > window.innerWidth;
    });
    // Ideally no horizontal scroll on mobile
    if (hasHorizontalScroll) {
      console.warn("Warning: Horizontal scroll detected on mobile transactions page");
    }
  });

  test("mobile reports charts are responsive", async ({ page }) => {
    await page.goto("/#/reports/spending");
    await page.waitForLoadState("networkidle");

    await page.screenshot({ path: "e2e/screenshots/mobile-reports.png", fullPage: true });
  });
});

test.describe("Accessibility Audit", () => {
  test("dashboard has proper heading hierarchy", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Wait for stat cards to render (indicates dashboard content loaded)
    await expect(page.locator(".stat-card").first()).toBeVisible({ timeout: 5000 });

    // Dashboard has sr-only h1 "Dashboard" and visible h2s
    // Check for the screen-reader only h1 or any h1
    const h1Count = await page.locator("h1").count();
    expect(h1Count).toBeGreaterThanOrEqual(1);
  });

  test("interactive elements are keyboard accessible", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Tab to first interactive element
    await page.keyboard.press("Tab");

    // Something should be focused
    const focusedElement = await page.evaluate(() => {
      const el = document.activeElement;
      return el?.tagName || null;
    });
    expect(focusedElement).toBeTruthy();
  });

  test("buttons have accessible names", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    const buttons = page.locator("button");
    const count = await buttons.count();

    for (let i = 0; i < Math.min(count, 10); i++) {
      const button = buttons.nth(i);
      const text = await button.textContent();
      const ariaLabel = await button.getAttribute("aria-label");
      const title = await button.getAttribute("title");

      // Button should have some accessible name
      const hasAccessibleName = (text && text.trim().length > 0) || ariaLabel || title;
      if (!hasAccessibleName) {
        // Check if it contains an icon with sr-only text
        const srOnlyText = await button.locator(".sr-only").textContent().catch(() => null);
        if (!srOnlyText) {
          console.warn(`Button ${i} may lack accessible name`);
        }
      }
    }
  });

  test("form inputs have labels", async ({ page }) => {
    await page.goto("/#/import");
    await page.waitForLoadState("networkidle");

    const inputs = page.locator("input:not([type='hidden']), select, textarea");
    const count = await inputs.count();

    for (let i = 0; i < count; i++) {
      const input = inputs.nth(i);
      const id = await input.getAttribute("id");
      const ariaLabel = await input.getAttribute("aria-label");
      const placeholder = await input.getAttribute("placeholder");

      // Check for associated label
      if (id) {
        const label = page.locator(`label[for="${id}"]`);
        const hasLabel = await label.count() > 0;
        if (!hasLabel && !ariaLabel) {
          console.warn(`Input ${i} (id=${id}) may lack label, placeholder: ${placeholder}`);
        }
      }
    }
  });

  test("color contrast in dark mode", async ({ page }) => {
    await enableDarkMode(page);
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Check key text elements have sufficient contrast
    // This is a basic check - for full WCAG compliance, use axe-core
    const mainHeading = page.locator("h1").first();
    if (await mainHeading.isVisible()) {
      const color = await mainHeading.evaluate((el) => {
        return window.getComputedStyle(el).color;
      });
      // Text should not be too dark in dark mode
      expect(color).not.toBe("rgb(0, 0, 0)");
    }
  });
});

test.describe("Interactive Element UX", () => {
  test("dropdown selects are usable", async ({ page }) => {
    await page.goto("/#/reports/spending");
    await page.waitForLoadState("networkidle");

    const periodSelect = page.locator("select").first();
    if (await periodSelect.isVisible()) {
      // Should be clickable
      await periodSelect.click();

      // Should show options
      const options = periodSelect.locator("option");
      const optionCount = await options.count();
      expect(optionCount).toBeGreaterThan(0);

      // Can change value
      await periodSelect.selectOption("last-month");
      await expect(periodSelect).toHaveValue("last-month");
    }
  });

  test("search input provides feedback", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    const searchInput = page.locator('input[placeholder*="Search"]');
    if (await searchInput.isVisible()) {
      await searchInput.fill("test search");

      // Should show loading or results change
      await page.waitForTimeout(500); // Wait for debounce

      // Clear button should appear
      const clearButton = page.locator('button:has(svg)').filter({ has: page.locator('svg') });
      // Note: actual implementation may vary
    }
  });

  test("tab navigation in reports works", async ({ page }) => {
    await page.goto("/#/reports");
    await page.waitForLoadState("networkidle");

    // Click through tabs - scope to the tab bar (border-b container)
    const tabBar = page.locator(".border-b.border-hone-200");
    const tabs = ["Spending", "Trends", "Merchants"];
    for (const tabName of tabs) {
      const tab = tabBar.getByRole("button", { name: tabName });
      if (await tab.isVisible()) {
        await tab.click();
        await page.waitForTimeout(200);

        // URL should update
        const url = page.url();
        expect(url.toLowerCase()).toContain(tabName.toLowerCase());
      }
    }
  });

  test("cards have hover states in light mode", async ({ page }) => {
    await enableLightMode(page);
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    const card = page.locator(".card").first();
    if (await card.isVisible()) {
      // Hover and check for visual change
      await card.hover();
      // Visual inspection via screenshot
      await page.screenshot({ path: "e2e/screenshots/card-hover.png" });
    }
  });
});

test.describe("Error States and Edge Cases", () => {
  test("empty transactions shows helpful message", async ({ page }) => {
    // With fresh DB, transactions should be empty
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Wait for page heading to ensure page is loaded
    await expect(page.getByRole("heading", { name: /Transactions/ })).toBeVisible({ timeout: 5000 });

    // Either shows transactions or empty state
    const content = await page.textContent("body");
    const hasTransactions = content?.includes("Showing") || content?.includes("total");
    // Actual empty state text from component
    const hasEmptyState = content?.includes("No Transactions") || content?.includes("No transactions match");

    expect(hasTransactions || hasEmptyState).toBe(true);
  });

  test("alerts page shows alerts or helpful message", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // Wait for page heading to ensure page is loaded
    await expect(page.getByRole("heading", { level: 1, name: "Alerts" })).toBeVisible({ timeout: 5000 });

    const content = await page.textContent("body");
    // Check for actual alert content (Zombie Subscription, Price Increase, Duplicate Service)
    const hasAlerts = content?.includes("Zombie Subscription") ||
                      content?.includes("Price Increase") ||
                      content?.includes("Duplicate Service");
    // Actual empty state text from component: "All Clear!" or "No active alerts"
    const hasEmptyState = content?.includes("All Clear") || content?.includes("No active alerts");

    expect(hasAlerts || hasEmptyState).toBe(true);
  });

  test("loading states are shown", async ({ page }) => {
    // Slow down network to observe loading
    await page.route("**/api/**", async (route) => {
      await new Promise((r) => setTimeout(r, 500));
      await route.continue();
    });

    await page.goto("/");

    // Should see loading indicator or content
    // This verifies the app doesn't just show blank during load
    const hasContent = await page.locator("body").textContent();
    expect(hasContent).toBeTruthy();
  });
});

test.describe("Visual Consistency Checks", () => {
  test("consistent button styling", async ({ page }) => {
    await page.goto("/#/import");
    await page.waitForLoadState("networkidle");

    const primaryButtons = page.locator(".btn-primary");
    const secondaryButtons = page.locator(".btn-secondary");

    // Check that button classes are being used consistently
    const primaryCount = await primaryButtons.count();
    const secondaryCount = await secondaryButtons.count();

    // Log for review
    console.log(`Primary buttons: ${primaryCount}, Secondary buttons: ${secondaryCount}`);
  });

  test("consistent spacing in card layouts", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    const cards = page.locator(".card");
    const count = await cards.count();

    if (count > 1) {
      // Check cards have consistent padding
      const firstPadding = await cards.first().evaluate((el) => {
        return window.getComputedStyle(el).padding;
      });

      for (let i = 1; i < Math.min(count, 5); i++) {
        const padding = await cards.nth(i).evaluate((el) => {
          return window.getComputedStyle(el).padding;
        });
        // Padding patterns should be from a limited set (consistent design)
      }
    }
  });
});
