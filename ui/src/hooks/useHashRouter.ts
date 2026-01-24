import { useState, useEffect, useCallback } from "react";

export type View = "dashboard" | "transactions" | "subscriptions" | "alerts" | "reports" | "tags" | "receipts" | "import" | "history" | "ai-metrics" | "feedback" | "explore";

const VALID_VIEWS: View[] = ["dashboard", "transactions", "subscriptions", "alerts", "reports", "tags", "receipts", "import", "history", "ai-metrics", "feedback", "explore"];

export interface RouterState {
  view: View;
  subview: string | null;
  params: Record<string, string>;
}

function parseHash(hash: string): RouterState {
  // Remove leading # and /
  let cleanHash = hash.startsWith("#") ? hash.slice(1) : hash;
  if (cleanHash.startsWith("/")) cleanHash = cleanHash.slice(1);

  // Split path and query
  const [pathPart, queryPart] = cleanHash.split("?");
  const pathSegments = pathPart.split("/").filter(Boolean);

  // Parse view and subview, validate view
  const rawView = pathSegments[0];
  const view: View = VALID_VIEWS.includes(rawView as View) ? (rawView as View) : "dashboard";
  const subview = pathSegments[1] || null;

  // Parse query params
  const params: Record<string, string> = {};
  if (queryPart) {
    const searchParams = new URLSearchParams(queryPart);
    searchParams.forEach((value, key) => {
      params[key] = value;
    });
  }

  return { view, subview, params };
}

function buildHash(view: View, subview?: string | null, params?: Record<string, string>): string {
  let hash = `#/${view}`;
  if (subview) hash += `/${subview}`;

  if (params && Object.keys(params).length > 0) {
    const searchParams = new URLSearchParams();
    // Sort keys for consistent URLs
    Object.keys(params).sort().forEach((key) => {
      const value = params[key];
      if (value) searchParams.set(key, value);
    });
    const query = searchParams.toString();
    if (query) hash += `?${query}`;
  }

  return hash;
}

export function useHashRouter() {
  const [state, setState] = useState<RouterState>(() => parseHash(window.location.hash));

  useEffect(() => {
    const handleHashChange = () => {
      setState(parseHash(window.location.hash));
    };

    window.addEventListener("hashchange", handleHashChange);
    return () => window.removeEventListener("hashchange", handleHashChange);
  }, []);

  const navigate = useCallback((view: View, subview?: string | null, params?: Record<string, string>) => {
    window.location.hash = buildHash(view, subview, params);
  }, []);

  const updateParams = useCallback((newParams: Record<string, string>) => {
    const current = parseHash(window.location.hash);
    window.location.hash = buildHash(current.view, current.subview, { ...current.params, ...newParams });
  }, []);

  const setParams = useCallback((params: Record<string, string>) => {
    const current = parseHash(window.location.hash);
    window.location.hash = buildHash(current.view, current.subview, params);
  }, []);

  const setSubview = useCallback((subview: string | null) => {
    const current = parseHash(window.location.hash);
    window.location.hash = buildHash(current.view, subview, current.params);
  }, []);

  return { state, navigate, updateParams, setParams, setSubview };
}
