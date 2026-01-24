import { execSync } from "child_process";
import { existsSync, unlinkSync } from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TEST_DB = process.env.TEST_DB || "/tmp/hone-ux-test.db";

async function globalSetup() {
  // In CI, the database is seeded by the workflow before running tests
  // This avoids duplicate seeding and uses the pre-built binary
  if (process.env.CI && existsSync(TEST_DB)) {
    console.log("🌱 Using pre-seeded test database (CI mode)");
    return;
  }

  console.log("🌱 Setting up UX test database...");

  // Remove existing test database for clean state in local dev
  if (existsSync(TEST_DB)) {
    console.log("  Removing existing test database...");
    unlinkSync(TEST_DB);
  }

  const projectRoot = path.resolve(__dirname, "../..");
  const honeBin = process.env.CI ? "./target/release/hone" : "cargo run --";

  // Use login shell to get PATH with cargo
  const execOptions = {
    cwd: projectRoot,
    stdio: "inherit" as const,
    shell: "/bin/bash",
    env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` },
  };

  try {
    // Initialize database
    console.log("  Initializing database...");
    execSync(`${honeBin} --no-encrypt --db ${TEST_DB} init`, execOptions);

    // Import test data - this runs tagging and detection automatically
    console.log("  Importing test transactions (with tagging and detection)...");
    execSync(
      `${honeBin} --no-encrypt --db ${TEST_DB} import --file samples/test_data.csv --bank chase`,
      execOptions
    );

    console.log("✅ UX test database seeded successfully!");
  } catch (error) {
    console.error("❌ Failed to seed test database:", error);
    throw error;
  }
}

export default globalSetup;
