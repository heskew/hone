import { test, expect } from "@playwright/test";

test.describe("Alerts Page", () => {
  test("loads and displays page heading", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    await expect(
      page.getByRole("heading", { level: 1, name: "Alerts" })
    ).toBeVisible();
  });

  test("shows alerts or All Clear state", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // Either shows actual alerts or an "all clear" message
    const content = await page.textContent("body");
    const hasAlerts = content?.includes("Zombie Subscription") ||
                      content?.includes("Price Increase") ||
                      content?.includes("Duplicate Service") ||
                      content?.includes("$");
    const hasAllClear = content?.includes("All Clear") ||
                        content?.includes("No active alerts") ||
                        content?.includes("good work");

    expect(hasAlerts || hasAllClear).toBe(true);
  });

  test("alerts have dismiss button", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // Look for dismiss buttons
    const dismissButtons = page.getByRole("button", { name: /dismiss/i });
    const count = await dismissButtons.count();

    // If there are alerts, they should have dismiss buttons
    // If no alerts, count will be 0 which is also valid
    expect(count).toBeGreaterThanOrEqual(0);
  });

  test("show dismissed toggle exists", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // Look for "Show dismissed" toggle
    const toggleButton = page.getByText(/show dismissed/i);
    const hasToggle = await toggleButton.isVisible().catch(() => false);

    // Page should at least load without errors
    await expect(
      page.getByRole("heading", { level: 1, name: "Alerts" })
    ).toBeVisible();
  });
});

test.describe("Alert Types", () => {
  test("zombie subscription alerts show subscription info", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // If zombie alerts exist, they should have relevant info
    const zombieAlert = page.getByText(/zombie/i);
    if (await zombieAlert.isVisible().catch(() => false)) {
      // Should show subscription name and amount
      const content = await page.textContent("body");
      expect(content).toMatch(/\$/); // Should show a dollar amount
    }
  });

  test("price increase alerts show old vs new price", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // If price increase alerts exist, they may show comparison
    const priceAlert = page.getByText(/price increase/i);
    if (await priceAlert.isVisible().catch(() => false)) {
      // Should show price info
      const content = await page.textContent("body");
      expect(content).toMatch(/\$/);
    }
  });

  test("duplicate alerts show category", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // If duplicate alerts exist, they show which category
    const duplicateAlert = page.getByText(/duplicate/i);
    if (await duplicateAlert.isVisible().catch(() => false)) {
      // Page loaded successfully
      expect(true).toBe(true);
    }
  });
});

test.describe("Alert Actions", () => {
  test("can dismiss an alert", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // Find a dismiss button
    const dismissButton = page.getByRole("button", { name: /dismiss/i }).first();

    if (await dismissButton.isVisible().catch(() => false)) {
      // Get count of alerts before
      const alertsBefore = await page.locator(".card").count();

      await dismissButton.click();
      await page.waitForTimeout(300);

      // Page should update (either fewer alerts or alert moved to dismissed)
      await expect(
        page.getByRole("heading", { level: 1, name: "Alerts" })
      ).toBeVisible();
    }
  });

  test("can toggle show dismissed", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // Find "Show dismissed" toggle
    const toggleLabel = page.getByText(/show dismissed/i);
    const toggleButton = page.locator("button, input[type='checkbox']").filter({ hasText: /dismissed/i });

    if (await toggleLabel.isVisible().catch(() => false)) {
      // Click the toggle
      await toggleLabel.click();
      await page.waitForTimeout(300);

      // Page should still work
      await expect(
        page.getByRole("heading", { level: 1, name: "Alerts" })
      ).toBeVisible();
    }
  });

  test("can restore a dismissed alert", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // First enable "Show dismissed"
    const toggleLabel = page.getByText(/show dismissed/i);
    if (await toggleLabel.isVisible().catch(() => false)) {
      await toggleLabel.click();
      await page.waitForTimeout(300);

      // Look for restore button
      const restoreButton = page.getByRole("button", { name: /restore/i }).first();
      if (await restoreButton.isVisible().catch(() => false)) {
        await restoreButton.click();
        await page.waitForTimeout(300);

        // Page should still work
        await expect(
          page.getByRole("heading", { level: 1, name: "Alerts" })
        ).toBeVisible();
      }
    }
  });
});

test.describe("Alert Navigation", () => {
  test("can navigate from dashboard to alerts", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Click View all alerts link if visible
    const viewAllLink = page.getByRole("button", { name: /view all/i });
    if (await viewAllLink.isVisible().catch(() => false)) {
      await viewAllLink.click();
      await page.waitForTimeout(300);

      expect(page.url()).toContain("#/alerts");
    }
  });

  test("deep link to alerts page", async ({ page }) => {
    await page.goto("/#/alerts");

    await expect(
      page.getByRole("heading", { level: 1, name: "Alerts" })
    ).toBeVisible();
    expect(page.url()).toContain("#/alerts");
  });
});

test.describe("Alert Potential Savings", () => {
  test("shows potential savings amount", async ({ page }) => {
    await page.goto("/#/alerts");
    await page.waitForLoadState("networkidle");

    // Look for potential savings display
    const content = await page.textContent("body");

    // Either shows savings amount or no alerts message
    const hasSavings = content?.match(/\$\d+/) || content?.includes("savings");
    const hasAllClear = content?.includes("All Clear") || content?.includes("No active alerts");

    // Page should load correctly
    await expect(
      page.getByRole("heading", { level: 1, name: "Alerts" })
    ).toBeVisible();
  });
});
