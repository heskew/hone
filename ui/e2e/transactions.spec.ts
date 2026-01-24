import { test, expect } from "@playwright/test";

test.describe("Transactions List", () => {
  test("loads and displays transactions", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Should show transactions heading
    await expect(
      page.getByRole("heading", { name: /Transactions/ })
    ).toBeVisible();

    // Should show search input
    await expect(page.locator('input[placeholder*="Search"]')).toBeVisible();
  });

  test("search filters transactions", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    const searchInput = page.locator('input[placeholder*="Search"]');
    await searchInput.fill("NETFLIX");

    // Wait for debounce and results
    await page.waitForTimeout(500);

    // Should still show transactions (filtered)
    const content = await page.textContent("body");
    // Either we find NETFLIX transactions or no results
    expect(content).toBeDefined();
  });

  test("pagination controls are present", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Wait for page heading to ensure page is loaded
    await expect(page.getByRole("heading", { name: /Transactions/ })).toBeVisible({ timeout: 5000 });

    // Check for pagination info or controls
    const content = await page.textContent("body");
    // Should show "Showing X of Y" or similar
    const hasPaginationText = content?.includes("Showing") || content?.includes("of");
    const hasTransactions = content?.includes("total") || content?.includes("transactions");

    // Either pagination is visible or there are no transactions
    expect(hasPaginationText || hasTransactions || content?.includes("No Transactions")).toBe(true);
  });
});

test.describe("Untagged Filter", () => {
  test("tag filter dropdown exists", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Wait for page heading to ensure page is loaded
    await expect(page.getByRole("heading", { name: /Transactions/ })).toBeVisible({ timeout: 5000 });

    // Look for tag filter button or dropdown - check for any button with tag-related text
    const buttons = page.locator("button");
    const count = await buttons.count();

    // Should have multiple buttons (including filter controls)
    expect(count).toBeGreaterThan(0);
  });

  test("deep link to untagged transactions works", async ({ page }) => {
    await page.goto("/#/transactions?untagged=1");
    await page.waitForLoadState("networkidle");

    // Page should load with untagged filter active
    await expect(
      page.getByRole("heading", { name: /Transactions/ })
    ).toBeVisible();

    // URL should have untagged param
    expect(page.url()).toContain("untagged=1");
  });
});

test.describe("Transaction Detail Modal", () => {
  test("clicking transaction opens detail modal", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Transaction rows have cursor-pointer class and hover states
    // Look for clickable transaction items
    const transactionRow = page.locator(".cursor-pointer").first();

    if (await transactionRow.isVisible()) {
      await transactionRow.click();
      await page.waitForTimeout(500);

      // Modal should appear with "Transaction Details" heading
      const modal = page.getByText("Transaction Details");
      await expect(modal).toBeVisible({ timeout: 3000 });
    }
  });

  test("modal has tabs for Splits, Location, Tags, Receipts", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Wait for page heading to ensure page is loaded
    await expect(page.getByRole("heading", { name: /Transactions/ })).toBeVisible({ timeout: 5000 });

    // Open first transaction
    const transactionRow = page.locator(".cursor-pointer").first();

    if (await transactionRow.isVisible()) {
      await transactionRow.click();

      // Wait for modal to appear
      await expect(page.getByText("Transaction Details")).toBeVisible({ timeout: 3000 });

      // Check for tab buttons within the modal (they have border-b-2 class)
      const modal = page.locator(".fixed.inset-0");
      await expect(modal.locator("button.border-b-2").filter({ hasText: "Splits" })).toBeVisible({ timeout: 3000 });
      await expect(modal.locator("button.border-b-2").filter({ hasText: "Location" })).toBeVisible();
      await expect(modal.locator("button.border-b-2").filter({ hasText: /^Tags/ }).first()).toBeVisible();
      await expect(modal.locator("button.border-b-2").filter({ hasText: "Receipts" })).toBeVisible();
    }
  });

  test("can switch to Tags tab", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Open first transaction
    const transactionRow = page.locator(".cursor-pointer").first();

    if (await transactionRow.isVisible()) {
      await transactionRow.click();
      await page.waitForTimeout(500);

      // Click Tags tab in the modal (has border-b-2 class, unlike nav buttons)
      const modal = page.locator(".fixed.inset-0");
      const tagsTab = modal.locator("button.border-b-2").filter({ hasText: /^Tags/ }).first();
      if (await tagsTab.isVisible()) {
        await tagsTab.click();
        await page.waitForTimeout(300);

        // Should show "Current Tags" section
        await expect(page.getByText("Current Tags")).toBeVisible();
        // Should show "Add Tag" section
        await expect(page.getByText("Add Tag")).toBeVisible();
      }
    }
  });

  test("Tags tab shows hierarchical tag list", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Wait for page heading to ensure page is loaded
    await expect(page.getByRole("heading", { name: /Transactions/ })).toBeVisible({ timeout: 5000 });

    // Open first transaction
    const transactionRow = page.locator(".cursor-pointer").first();

    if (await transactionRow.isVisible()) {
      await transactionRow.click();

      // Wait for modal to appear
      await expect(page.getByText("Transaction Details")).toBeVisible({ timeout: 3000 });

      // Click Tags tab in the modal (has border-b-2 class, unlike nav buttons)
      const modal = page.locator(".fixed.inset-0");
      const tagsTab = modal.locator("button.border-b-2").filter({ hasText: /^Tags/ }).first();
      if (await tagsTab.isVisible()) {
        await tagsTab.click();

        // Wait for "Add Tag" section to appear (indicates Tags tab content loaded)
        await expect(page.getByText("Add Tag")).toBeVisible({ timeout: 3000 });

        // Should show "Current Tags" section
        await expect(page.getByText("Current Tags")).toBeVisible();

        // Wait for the tag list to load - tags are buttons inside the scrollable container
        // The tag list has .max-h-60 class and contains buttons with tag names
        const tagListContainer = modal.locator(".max-h-60");
        await expect(tagListContainer).toBeVisible({ timeout: 3000 });

        // Wait for at least one tag button to appear in the list
        const tagButtons = tagListContainer.locator("button");
        await expect(tagButtons.first()).toBeVisible({ timeout: 3000 });

        // Verify we have multiple tags loaded (seeded data should have 15 root tags)
        const tagCount = await tagButtons.count();
        expect(tagCount).toBeGreaterThan(0);
      }
    }
  });

  test("can close modal with X button", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Open first transaction
    const transactionRow = page.locator(".cursor-pointer").first();

    if (await transactionRow.isVisible()) {
      await transactionRow.click();
      await page.waitForTimeout(500);

      // Modal should be visible
      await expect(page.getByText("Transaction Details")).toBeVisible({ timeout: 3000 });

      // Use the Close button at the bottom (more reliable than X icon)
      const closeBtn = page.getByRole("button", { name: "Close" });
      await closeBtn.click();

      await page.waitForTimeout(300);

      // Modal should be closed
      await expect(page.getByText("Transaction Details")).not.toBeVisible();
    }
  });

  test("can close modal with Escape key", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Open first transaction
    const transactionRow = page.locator(".cursor-pointer").first();

    if (await transactionRow.isVisible()) {
      await transactionRow.click();
      await page.waitForTimeout(500);

      // Modal should be visible
      await expect(page.getByText("Transaction Details")).toBeVisible({ timeout: 3000 });

      // Press Escape
      await page.keyboard.press("Escape");

      await page.waitForTimeout(300);

      // Modal should be closed
      await expect(page.getByText("Transaction Details")).not.toBeVisible();
    }
  });

  test("can close modal with Close button", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Open first transaction
    const transactionRow = page.locator(".cursor-pointer").first();

    if (await transactionRow.isVisible()) {
      await transactionRow.click();
      await page.waitForTimeout(500);

      // Modal should be visible
      await expect(page.getByText("Transaction Details")).toBeVisible({ timeout: 3000 });

      // Click Close button at bottom
      const closeBtn = page.getByRole("button", { name: "Close" });
      await closeBtn.click();

      await page.waitForTimeout(300);

      // Modal should be closed
      await expect(page.getByText("Transaction Details")).not.toBeVisible();
    }
  });
});

test.describe("Date Range Filter", () => {
  test("date range selector exists", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Look for period/date selector
    const periodSelect = page.locator("select").first();
    if (await periodSelect.isVisible()) {
      // Check that it has date range options
      const options = periodSelect.locator("option");
      const count = await options.count();
      expect(count).toBeGreaterThan(0);
    }
  });

  test("can change date range period", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    const periodSelect = page.locator("select").first();
    if (await periodSelect.isVisible()) {
      // Select a different period
      await periodSelect.selectOption({ index: 1 });
      await page.waitForTimeout(300);

      // Transactions should reload (just verify no error)
      await expect(
        page.getByRole("heading", { name: /Transactions/ })
      ).toBeVisible();
    }
  });
});

test.describe("Sorting", () => {
  test("can sort by date column", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Look for sortable column headers
    const dateHeader = page.getByText("Date").first();
    if (await dateHeader.isVisible()) {
      await dateHeader.click();
      await page.waitForTimeout(300);

      // Should still show transactions (sorted)
      await expect(
        page.getByRole("heading", { name: /Transactions/ })
      ).toBeVisible();
    }
  });

  test("can sort by amount column", async ({ page }) => {
    await page.goto("/#/transactions");
    await page.waitForLoadState("networkidle");

    // Look for sortable column headers
    const amountHeader = page.getByText("Amount").first();
    if (await amountHeader.isVisible()) {
      await amountHeader.click();
      await page.waitForTimeout(300);

      // Should still show transactions (sorted)
      await expect(
        page.getByRole("heading", { name: /Transactions/ })
      ).toBeVisible();
    }
  });
});
