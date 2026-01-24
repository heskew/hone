import syntaxHighlight from "@11ty/eleventy-plugin-syntaxhighlight";
import markdownItAnchor from "markdown-it-anchor";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default function(eleventyConfig) {
  // Default layout for all pages
  eleventyConfig.addGlobalData("layout", "base.njk");

  // Syntax highlighting for code blocks
  eleventyConfig.addPlugin(syntaxHighlight);

  // Copy static assets - use absolute paths
  eleventyConfig.addPassthroughCopy({ [path.join(__dirname, "theme")]: "theme" });
  eleventyConfig.addPassthroughCopy({ [path.join(__dirname, "favicon.svg")]: "favicon.svg" });
  eleventyConfig.addPassthroughCopy({ [path.join(__dirname, "..", "llms.txt")]: "llms.txt" });

  // Markdown configuration with anchor links
  eleventyConfig.amendLibrary("md", (mdLib) => {
    mdLib.use(markdownItAnchor, {
      permalink: markdownItAnchor.permalink.headerLink(),
      level: [2, 3, 4]
    });
  });

  // Collection for navigation - all pages sorted by title
  eleventyConfig.addCollection("docs", function(collectionApi) {
    return collectionApi.getFilteredByGlob(["**/*.md", "!site/**", "!node_modules/**"])
      .filter(item => item.data.title)
      .sort((a, b) => {
        const orderA = a.data.nav_order ?? 999;
        const orderB = b.data.nav_order ?? 999;
        if (orderA !== orderB) return orderA - orderB;
        return (a.data.title || "").localeCompare(b.data.title || "");
      });
  });

  // Design docs subcollection
  eleventyConfig.addCollection("design", function(collectionApi) {
    return collectionApi.getFilteredByGlob("design/**/*.md")
      .filter(item => item.data.title)
      .sort((a, b) => {
        const orderA = a.data.nav_order ?? 999;
        const orderB = b.data.nav_order ?? 999;
        if (orderA !== orderB) return orderA - orderB;
        return (a.data.title || "").localeCompare(b.data.title || "");
      });
  });

  // Format date filter - use UTC to avoid timezone shifts
  eleventyConfig.addFilter("formatDate", (date) => {
    if (!date) return "";
    const d = new Date(date);
    return d.toLocaleDateString("en-US", {
      year: "numeric",
      month: "long",
      day: "numeric",
      timeZone: "UTC"
    });
  });

  return {
    dir: {
      input: path.join(__dirname, ".."),
      output: path.join(__dirname, "_site"),
      includes: "site/_includes",
      layouts: "site/_layouts",
      data: "site/_data"
    },
    markdownTemplateEngine: "njk",
    htmlTemplateEngine: "njk"
  };
}
