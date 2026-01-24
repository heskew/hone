import { defineConfig, devices } from "@playwright/test";
import { fileURLToPath } from "url";
import path from "path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TEST_DB = process.env.TEST_DB || "/tmp/hone-ux-test.db";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 4 : undefined,
  reporter: process.env.CI ? [["github"], ["html"]] : "html",
  // Run global setup to seed test database before tests
  globalSetup: path.resolve(__dirname, "./e2e/global-setup.ts"),
  use: {
    baseURL: "http://localhost:5173",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: [
    {
      // Use seeded test database for consistent UX testing
      // In CI, use pre-built release binary; locally, use cargo run
      command: process.env.CI
        ? `../target/release/hone --no-encrypt --db ${TEST_DB} serve --port 3000 --no-auth`
        : `cd .. && cargo run -- --no-encrypt --db ${TEST_DB} serve --port 3000 --no-auth`,
      url: "http://localhost:3000/api/dashboard",
      reuseExistingServer: !process.env.CI,
      timeout: process.env.CI ? 30000 : 120000,
    },
    {
      command: "npm run dev",
      url: "http://localhost:5173",
      reuseExistingServer: !process.env.CI,
      timeout: 30000,
    },
  ],
});
