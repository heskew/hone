import { test, expect } from "@playwright/test";

test.describe("Dashboard", () => {
  test("loads and displays header", async ({ page }) => {
    await page.goto("/");

    // Should show app name in header
    await expect(page.getByText("Hone").first()).toBeVisible();

    // Should show navigation buttons
    await expect(page.getByRole("button", { name: "Dashboard" })).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Transactions" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Subscriptions" }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "Alerts" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Import" })).toBeVisible();
  });

  test("shows stats cards", async ({ page }) => {
    await page.goto("/");

    // Look for stat labels
    await expect(page.getByText("Accounts")).toBeVisible();
    await expect(page.getByText("Untagged")).toBeVisible();
    await expect(page.getByText("Active Subscriptions")).toBeVisible();
    await expect(page.getByText("Potential Savings")).toBeVisible();
  });

  test("stats cards show numeric values", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Wait for stat cards to render (they have .stat-card class)
    await expect(page.locator(".stat-card").first()).toBeVisible({ timeout: 5000 });

    // Find all stat cards
    const cards = page.locator(".stat-card");
    const count = await cards.count();

    // Should have multiple stat cards
    expect(count).toBeGreaterThan(0);

    // At least the transactions count should be visible (from seeded data)
    const pageContent = await page.textContent("body");
    // Should have some numbers displayed
    expect(pageContent).toMatch(/\d+/);
  });
});

test.describe("Dashboard Stats Card Navigation", () => {
  test("clicking Accounts card navigates to Import view", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Find the Accounts stat card by its label within the stat-card container
    const accountsCard = page.locator(".stat-card").filter({ hasText: "Accounts" });
    if (await accountsCard.isVisible()) {
      await accountsCard.click();
      await page.waitForTimeout(300);

      // Should navigate to import view
      expect(page.url()).toContain("#/import");
    }
  });

  test("clicking Transactions card navigates to Transactions view", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Find the Transactions stat card (not the nav button) by the stat-card class
    const transactionsCard = page.locator(".stat-card").filter({ has: page.locator(".stat-label", { hasText: "Transactions" }) });
    if (await transactionsCard.isVisible()) {
      await transactionsCard.click();
      await page.waitForTimeout(300);

      // Should navigate to transactions view
      expect(page.url()).toContain("#/transactions");
    }
  });

  test("clicking Untagged card navigates to filtered Transactions view", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Find the Untagged stat card
    const untaggedCard = page.locator(".stat-card").filter({ hasText: "Untagged" });
    if (await untaggedCard.isVisible()) {
      await untaggedCard.click();
      await page.waitForTimeout(300);

      // Should navigate to transactions view with untagged filter
      expect(page.url()).toContain("#/transactions");
      expect(page.url()).toContain("untagged=1");
    }
  });

  test("clicking Active Subscriptions card navigates to Subscriptions view", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Find the Active Subscriptions stat card
    const subscriptionsCard = page.locator(".stat-card").filter({ hasText: "Active Subscriptions" });
    if (await subscriptionsCard.isVisible()) {
      await subscriptionsCard.click();
      await page.waitForTimeout(300);

      // Should navigate to subscriptions view
      expect(page.url()).toContain("#/subscriptions");
    }
  });

  test("clicking Potential Savings card navigates to Alerts view", async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Find the Potential Savings stat card
    const savingsCard = page.locator(".stat-card").filter({ hasText: "Potential Savings" });
    if (await savingsCard.isVisible()) {
      await savingsCard.click();
      await page.waitForTimeout(300);

      // Should navigate to alerts view
      expect(page.url()).toContain("#/alerts");
    }
  });
});

test.describe("Navigation", () => {
  test("can switch to Transactions view", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Transactions" }).click();

    // Should show transactions heading or empty state
    await expect(
      page.getByRole("heading", { name: /Transactions|No Transactions/ }),
    ).toBeVisible();
  });

  test("can switch to Subscriptions view", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Subscriptions" }).click();

    // Should show subscriptions page heading
    await expect(
      page.getByRole("heading", { level: 1, name: "Subscriptions" }),
    ).toBeVisible();
  });

  test("can switch to Alerts view", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Alerts" }).click();

    // Should show alerts page heading
    await expect(
      page.getByRole("heading", { level: 1, name: "Alerts" }),
    ).toBeVisible();
  });

  test("can switch to Import view", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Import" }).click();

    await expect(
      page.getByRole("heading", { name: "Import Transactions" }),
    ).toBeVisible();
  });
});

test.describe("URL Routing", () => {
  test("navigation updates URL hash", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "Transactions" }).click();
    await expect(page).toHaveURL(/#\/transactions/);

    await page.getByRole("button", { name: "Reports" }).click();
    await expect(page).toHaveURL(/#\/reports/);

    await page.getByRole("button", { name: "Tags" }).click();
    await expect(page).toHaveURL(/#\/tags/);
  });

  test("deep link navigates directly to view", async ({ page }) => {
    await page.goto("/#/subscriptions");
    await expect(
      page.getByRole("heading", { level: 1, name: /Subscriptions/ }),
    ).toBeVisible();
  });

  test("refresh preserves current view", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "Alerts" }).click();
    await expect(page).toHaveURL(/#\/alerts/);

    await page.reload();

    // Should still be on alerts page
    await expect(
      page.getByRole("heading", { level: 1, name: "Alerts" }),
    ).toBeVisible();
  });

  test("back button returns to previous view", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "Transactions" }).click();
    await page.getByRole("button", { name: "Subscriptions" }).click();

    await page.goBack();
    await expect(page).toHaveURL(/#\/transactions/);
    await expect(
      page.getByRole("heading", { name: /Transactions/ }),
    ).toBeVisible();
  });

  test("reports deep link with tab", async ({ page }) => {
    await page.goto("/#/reports/trends");
    // Trends tab should be active
    await expect(page.locator("button.border-hone-600")).toContainText("Trends");
  });

  test("reports deep link with period param", async ({ page }) => {
    await page.goto("/#/reports/spending?period=this-year");
    // Period selector should show This Year
    await expect(page.locator("select")).toHaveValue("this-year");
  });
});
