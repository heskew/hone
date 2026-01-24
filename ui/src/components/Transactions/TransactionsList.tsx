import { ArrowDown, ArrowUp, Check, ChevronDown, Receipt, RefreshCw, Search, Square, SquareCheck, Tag, X } from "lucide-react";
import { useEffect, useState, useRef, useCallback } from "react";
import { api } from "../../api";
import type { Entity, Period, TagWithPath, Transaction } from "../../types";
import { TransactionItem } from "./TransactionItem";
import { useHashRouter } from "../../hooks";
import { BulkTagModal } from "./BulkTagModal";

interface TransactionsListProps {
  initialUntagged?: boolean;
}

const PERIODS: { value: Period | ""; label: string }[] = [
  { value: "", label: "All Time" },
  { value: "this-month", label: "This Month" },
  { value: "last-month", label: "Last Month" },
  { value: "last-30-days", label: "Last 30 Days" },
  { value: "last-90-days", label: "Last 90 Days" },
  { value: "this-year", label: "This Year" },
  { value: "last-year", label: "Last Year" },
  { value: "custom", label: "Custom Range" },
];

export function TransactionsList({ initialUntagged = false }: TransactionsListProps) {
  const { state: routerState, setParams } = useHashRouter();
  const urlParams = routerState.params;

  // Parse URL params for initial state
  const parseUrlState = useCallback(() => {
    return {
      search: urlParams.q || "",
      tagIds: urlParams.tags ? urlParams.tags.split(",").map(Number).filter(Boolean) : [],
      untagged: urlParams.untagged === "1" || initialUntagged,
      period: (urlParams.period || "") as Period | "",
      from: urlParams.from || "",
      to: urlParams.to || "",
      sort: (urlParams.sort || "date") as "date" | "amount",
      order: (urlParams.order || "desc") as "asc" | "desc",
      entityId: urlParams.entity ? Number(urlParams.entity) : null,
      cardMember: urlParams.card || null,
      page: urlParams.page ? Number(urlParams.page) : 0,
    };
  }, [urlParams, initialUntagged]);

  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [total, setTotal] = useState(0);
  const [tags, setTags] = useState<TagWithPath[]>([]);
  const [showTagDropdown, setShowTagDropdown] = useState(false);
  const [entities, setEntities] = useState<Entity[]>([]);
  const [cardMembers, setCardMembers] = useState<string[]>([]);
  const tagDropdownRef = useRef<HTMLDivElement>(null);
  const limit = 50;
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Local state synced with URL
  const urlState = parseUrlState();
  const [search, setSearch] = useState(urlState.search);
  const [debouncedSearch, setDebouncedSearch] = useState(urlState.search);
  const [selectedTagIds, setSelectedTagIds] = useState<number[]>(urlState.tagIds);
  const [showUntagged, setShowUntagged] = useState(urlState.untagged);
  const [period, setPeriod] = useState<Period | "">(urlState.period);
  const [customFrom, setCustomFrom] = useState(urlState.from);
  const [customTo, setCustomTo] = useState(urlState.to);
  const [appliedCustomRange, setAppliedCustomRange] = useState<{ from: string; to: string } | null>(
    urlState.from && urlState.to ? { from: urlState.from, to: urlState.to } : null
  );
  const [sortField, setSortField] = useState<"date" | "amount">(urlState.sort);
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">(urlState.order);
  const [selectedEntityId, setSelectedEntityId] = useState<number | null>(urlState.entityId);
  const [selectedCardMember, setSelectedCardMember] = useState<string | null>(urlState.cardMember);
  const [offset, setOffset] = useState(urlState.page * limit);

  // Selection mode state
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedTransactionIds, setSelectedTransactionIds] = useState<Set<number>>(new Set());
  const [showBulkTagModal, setShowBulkTagModal] = useState(false);
  const [bulkTagAction, setBulkTagAction] = useState<"add" | "remove">("add");

  // Selection handlers
  const toggleSelection = useCallback((id: number) => {
    setSelectedTransactionIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const selectAllOnPage = useCallback(() => {
    setSelectedTransactionIds((prev) => {
      const next = new Set(prev);
      for (const tx of transactions) {
        next.add(tx.id);
      }
      return next;
    });
  }, [transactions]);

  const clearSelection = useCallback(() => {
    setSelectedTransactionIds(new Set());
  }, []);

  const exitSelectionMode = useCallback(() => {
    setSelectionMode(false);
    setSelectedTransactionIds(new Set());
  }, []);

  // Check if all transactions on page are selected
  const allOnPageSelected = transactions.length > 0 &&
    transactions.every((tx) => selectedTransactionIds.has(tx.id));

  // Sync state to URL when filters change
  const syncToUrl = useCallback(() => {
    const params: Record<string, string> = {};
    if (debouncedSearch) params.q = debouncedSearch;
    if (selectedTagIds.length > 0) params.tags = selectedTagIds.join(",");
    if (showUntagged) params.untagged = "1";
    if (period && period !== "custom") params.period = period;
    if (appliedCustomRange) {
      params.from = appliedCustomRange.from;
      params.to = appliedCustomRange.to;
    }
    if (sortField !== "date") params.sort = sortField;
    if (sortOrder !== "desc") params.order = sortOrder;
    if (selectedEntityId) params.entity = String(selectedEntityId);
    if (selectedCardMember) params.card = selectedCardMember;
    if (offset > 0) params.page = String(Math.floor(offset / limit));
    setParams(params);
  }, [debouncedSearch, selectedTagIds, showUntagged, period, appliedCustomRange, sortField, sortOrder, selectedEntityId, selectedCardMember, offset, setParams]);

  // Sync to URL when filters change (debounced for search)
  useEffect(() => {
    syncToUrl();
  }, [debouncedSearch, selectedTagIds, showUntagged, period, appliedCustomRange, sortField, sortOrder, selectedEntityId, selectedCardMember, offset]);

  // Load tags and entities on mount
  useEffect(() => {
    api.getTagsTree().then(setTags).catch(console.error);
    api.getEntities({ entity_type: "person" }).then(setEntities).catch(console.error);
  }, []);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (tagDropdownRef.current && !tagDropdownRef.current.contains(e.target as Node)) {
        setShowTagDropdown(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Debounce search input
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(search);
      setOffset(0); // Reset to first page on new search
    }, 300);
    return () => clearTimeout(timer);
  }, [search]);

  // Handle period changes
  const handlePeriodChange = (newPeriod: Period | "") => {
    setPeriod(newPeriod);
    if (newPeriod !== "custom") {
      setAppliedCustomRange(null);
      setCustomFrom("");
      setCustomTo("");
    }
    setOffset(0);
  };

  const applyCustomRange = () => {
    if (customFrom && customTo) {
      setAppliedCustomRange({ from: customFrom, to: customTo });
      setOffset(0);
    }
  };

  useEffect(() => {
    loadTransactions();
  }, [offset, debouncedSearch, selectedTagIds, showUntagged, selectedEntityId, selectedCardMember, period, appliedCustomRange, sortField, sortOrder]);

  const loadTransactions = async () => {
    try {
      setLoading(true);
      const result = await api.getTransactions({
        limit,
        offset,
        entity_id: selectedEntityId || undefined,
        card_member: selectedCardMember || undefined,
        search: debouncedSearch || undefined,
        tag_ids: selectedTagIds.length > 0 ? selectedTagIds : undefined,
        untagged: showUntagged || undefined,
        period: period && period !== "custom" ? period : undefined,
        from: appliedCustomRange?.from,
        to: appliedCustomRange?.to,
        sort: sortField,
        order: sortOrder,
      });
      setTransactions(result.transactions);
      setTotal(result.total);

      // Extract unique card members from all transactions for the filter dropdown
      // Only update if we don't have any yet (first load without card_member filter)
      if (!selectedCardMember && cardMembers.length === 0) {
        const members = new Set<string>();
        result.transactions.forEach((t) => {
          if (t.card_member) members.add(t.card_member);
        });
        if (members.size > 0) {
          setCardMembers(Array.from(members).sort());
        }
      }
    } catch (err) {
      console.error("Failed to load transactions:", err);
    } finally {
      setLoading(false);
    }
  };

  const toggleTag = (tagId: number) => {
    setShowUntagged(false);
    setSelectedTagIds((prev) =>
      prev.includes(tagId) ? prev.filter((id) => id !== tagId) : [...prev, tagId]
    );
    setOffset(0);
  };

  const clearTagFilter = () => {
    setSelectedTagIds([]);
    setShowUntagged(false);
    setOffset(0);
  };

  // Flatten tags for display, including children with inherited colors
  const flattenTags = (tagList: TagWithPath[], depth = 0, inheritedColor?: string): { tag: TagWithPath; depth: number; effectiveColor?: string }[] => {
    const result: { tag: TagWithPath; depth: number; effectiveColor?: string }[] = [];
    for (const tag of tagList) {
      const effectiveColor = tag.color || inheritedColor;
      result.push({ tag, depth, effectiveColor });
      if (tag.children && tag.children.length > 0) {
        result.push(...flattenTags(tag.children, depth + 1, effectiveColor));
      }
    }
    return result;
  };

  const flatTags = flattenTags(tags);

  // Get selected tag names for display
  const selectedTagNames = selectedTagIds
    .map((id) => flatTags.find((t) => t.tag.id === id)?.tag.name)
    .filter(Boolean);

  const clearSearch = () => {
    setSearch("");
    searchInputRef.current?.focus();
  };

  const toggleSort = (field: "date" | "amount") => {
    if (sortField === field) {
      // Toggle order if same field
      setSortOrder(sortOrder === "desc" ? "asc" : "desc");
    } else {
      // Switch to new field with default desc order
      setSortField(field);
      setSortOrder("desc");
    }
    setOffset(0);
  };

  if (loading && transactions.length === 0 && !debouncedSearch) {
    return (
      <div className="card p-8 text-center">
        <RefreshCw className="w-8 h-8 text-hone-300 mx-auto mb-4 animate-spin" />
        <p className="text-hone-500">Loading transactions...</p>
      </div>
    );
  }

  const hasFilters = debouncedSearch || selectedTagIds.length > 0 || showUntagged || selectedEntityId || selectedCardMember || (period && period !== "custom") || appliedCustomRange;

  return (
    <div className="space-y-6 animate-fade-in">
      <div className="flex items-center justify-between flex-wrap gap-4">
        <h1 className="text-2xl font-bold">Transactions</h1>
        <div className="flex items-center gap-4">
          <span className="text-sm text-hone-500">{total.toLocaleString()} {hasFilters ? "matching" : "total"}</span>
          {selectionMode ? (
            <button
              onClick={exitSelectionMode}
              className="btn-secondary text-sm flex items-center gap-1.5"
            >
              <X className="w-4 h-4" />
              Done
            </button>
          ) : (
            <button
              onClick={() => setSelectionMode(true)}
              className="btn-secondary text-sm flex items-center gap-1.5"
            >
              <Check className="w-4 h-4" />
              Select
            </button>
          )}
        </div>
      </div>

      {/* Search and filter row */}
      <div className="flex gap-3 flex-wrap">
        {/* Search box */}
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-hone-400" />
          <input
            ref={searchInputRef}
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search by merchant or description..."
            className="w-full pl-10 pr-10 py-2 border border-hone-200 dark:border-hone-600 rounded-lg bg-white dark:bg-hone-800 text-hone-900 dark:text-hone-50 placeholder-hone-400 focus:outline-none focus:ring-2 focus:ring-hone-500"
          />
          {search && (
            <button
              onClick={clearSearch}
              className="absolute right-3 top-1/2 -translate-y-1/2 p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Tag filter dropdown */}
        <div className="relative" ref={tagDropdownRef}>
          <button
            onClick={() => setShowTagDropdown(!showTagDropdown)}
            className={`flex items-center gap-2 px-3 py-2 border rounded-lg transition-colors ${
              selectedTagIds.length > 0 || showUntagged
                ? "border-hone-500 bg-hone-100 dark:bg-hone-700 text-hone-900 dark:text-hone-50"
                : "border-hone-200 dark:border-hone-600 bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
            }`}
          >
            <Tag className="w-4 h-4" />
            {showUntagged ? (
              <span>Untagged</span>
            ) : selectedTagIds.length > 0 ? (
              <span className="max-w-[150px] truncate">
                {selectedTagNames.length === 1 ? selectedTagNames[0] : `${selectedTagIds.length} tags`}
              </span>
            ) : (
              <span>Filter by tag</span>
            )}
            <ChevronDown className={`w-4 h-4 transition-transform ${showTagDropdown ? "rotate-180" : ""}`} />
          </button>

          {showTagDropdown && (
            <div className="absolute right-0 mt-1 w-64 max-h-80 overflow-auto bg-white dark:bg-hone-800 border border-hone-200 dark:border-hone-600 rounded-lg shadow-lg z-20">
              {(selectedTagIds.length > 0 || showUntagged) && (
                <button
                  onClick={clearTagFilter}
                  className="w-full px-3 py-2 text-left text-sm text-hone-500 hover:bg-hone-50 dark:hover:bg-hone-700 border-b border-hone-100 dark:border-hone-700"
                >
                  Clear filter
                </button>
              )}
              {/* Untagged option */}
              <button
                onClick={() => {
                  setShowUntagged(!showUntagged);
                  setSelectedTagIds([]);
                  setOffset(0);
                }}
                className={`w-full px-3 py-2 text-left text-sm flex items-center gap-2 border-l-2 transition-colors border-b border-hone-100 dark:border-hone-700 ${
                  showUntagged
                    ? "border-l-hone-500 bg-hone-50 dark:bg-hone-700/50 font-medium"
                    : "border-l-transparent hover:border-l-hone-300 dark:hover:border-l-hone-500 hover:bg-hone-50 dark:hover:bg-hone-700/30"
                }`}
              >
                <span className="w-2.5 h-2.5 rounded-full flex-shrink-0 border border-hone-300 dark:border-hone-600" />
                <span className="flex-1 text-hone-500 dark:text-hone-400 italic">Untagged</span>
                {showUntagged && (
                  <span className="text-hone-500 dark:text-hone-400">✓</span>
                )}
              </button>
              {flatTags.map(({ tag, depth, effectiveColor }) => {
                const isSelected = selectedTagIds.includes(tag.id);
                return (
                  <button
                    key={tag.id}
                    onClick={() => toggleTag(tag.id)}
                    className={`w-full px-3 py-2 text-left text-sm flex items-center gap-2 border-l-2 transition-colors ${
                      isSelected
                        ? "border-l-hone-500 bg-hone-50 dark:bg-hone-700/50 font-medium"
                        : "border-l-transparent hover:border-l-hone-300 dark:hover:border-l-hone-500 hover:bg-hone-50 dark:hover:bg-hone-700/30"
                    }`}
                    style={{ paddingLeft: `${depth * 12 + 10}px` }}
                  >
                    {effectiveColor && (
                      <span
                        className="w-2.5 h-2.5 rounded-full flex-shrink-0"
                        style={{ backgroundColor: effectiveColor }}
                      />
                    )}
                    <span className="flex-1 truncate">{tag.name}</span>
                    {isSelected && (
                      <span className="text-hone-500 dark:text-hone-400">✓</span>
                    )}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* Entity (person) filter */}
        {entities.length > 0 && (
          <div className="flex items-center gap-2">
            <select
              value={selectedEntityId ?? ""}
              onChange={(e) => {
                setSelectedEntityId(e.target.value ? Number(e.target.value) : null);
                setOffset(0);
              }}
              className={`flex items-center gap-2 pl-3 pr-8 py-2 border rounded-lg transition-colors ${
                selectedEntityId
                  ? "border-hone-500 bg-hone-100 dark:bg-hone-700 text-hone-900 dark:text-hone-50"
                  : "border-hone-200 dark:border-hone-600 bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
              }`}
            >
              <option value="">All people</option>
              {entities.map((entity) => (
                <option key={entity.id} value={entity.id}>
                  {entity.name}
                </option>
              ))}
            </select>
            {selectedEntityId && (
              <button
                onClick={() => {
                  setSelectedEntityId(null);
                  setOffset(0);
                }}
                className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
                title="Clear person filter"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
        )}

        {/* Card member filter */}
        {cardMembers.length > 0 && (
          <div className="flex items-center gap-2">
            <select
              value={selectedCardMember ?? ""}
              onChange={(e) => {
                setSelectedCardMember(e.target.value || null);
                setOffset(0);
              }}
              className={`flex items-center gap-2 pl-3 pr-8 py-2 border rounded-lg transition-colors ${
                selectedCardMember
                  ? "border-hone-500 bg-hone-100 dark:bg-hone-700 text-hone-900 dark:text-hone-50"
                  : "border-hone-200 dark:border-hone-600 bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
              }`}
            >
              <option value="">All cardholders</option>
              {cardMembers.map((member) => (
                <option key={member} value={member}>
                  {member}
                </option>
              ))}
            </select>
            {selectedCardMember && (
              <button
                onClick={() => {
                  setSelectedCardMember(null);
                  setOffset(0);
                }}
                className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
                title="Clear cardholder filter"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
        )}

        {/* Period selector */}
        <div className="flex items-center gap-2">
          <select
            value={period}
            onChange={(e) => handlePeriodChange(e.target.value as Period | "")}
            className="pl-3 pr-8 py-2 border border-hone-200 dark:border-hone-600 rounded-lg bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
          >
            {PERIODS.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
          {period === "custom" && (
            <>
              <input
                type="date"
                value={customFrom}
                onChange={(e) => setCustomFrom(e.target.value)}
                className="px-3 py-2 border border-hone-200 dark:border-hone-600 rounded-lg bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
              />
              <span className="text-hone-400">to</span>
              <input
                type="date"
                value={customTo}
                onChange={(e) => setCustomTo(e.target.value)}
                className="px-3 py-2 border border-hone-200 dark:border-hone-600 rounded-lg bg-white dark:bg-hone-800 text-hone-600 dark:text-hone-300"
              />
              <button
                onClick={applyCustomRange}
                disabled={!customFrom || !customTo}
                className="btn-primary disabled:opacity-50"
              >
                Apply
              </button>
            </>
          )}
        </div>
      </div>

      {/* Active filter badges */}
      {selectedTagIds.length > 0 && (
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm text-hone-500">Filtering:</span>
          {selectedTagIds.map((id) => {
            const tagInfo = flatTags.find((t) => t.tag.id === id);
            return tagInfo ? (
              <span
                key={id}
                className="inline-flex items-center gap-1 px-2 py-1 text-sm rounded-full"
                style={{
                  backgroundColor: tagInfo.effectiveColor ? `${tagInfo.effectiveColor}20` : undefined,
                  color: tagInfo.effectiveColor || undefined,
                }}
              >
                {tagInfo.tag.name}
                <button
                  onClick={() => toggleTag(id)}
                  className="ml-1 hover:opacity-70"
                >
                  <X className="w-3 h-3" />
                </button>
              </span>
            ) : null;
          })}
          <button
            onClick={clearTagFilter}
            className="text-sm text-hone-500 hover:text-hone-700 dark:hover:text-hone-300"
          >
            Clear all
          </button>
        </div>
      )}

      {transactions.length === 0 && !loading ? (
        <div className="card p-8 text-center">
          {hasFilters ? (
            <>
              <Search className="w-12 h-12 text-hone-300 mx-auto mb-4" />
              <h2 className="text-lg font-semibold mb-2">No Results</h2>
              <p className="text-hone-500">
                No transactions match your {debouncedSearch && selectedTagIds.length > 0 ? "search and tag filter" : debouncedSearch ? "search" : "tag filter"}
              </p>
              <div className="flex gap-2 justify-center mt-4">
                {debouncedSearch && (
                  <button onClick={clearSearch} className="btn-secondary">
                    Clear Search
                  </button>
                )}
                {selectedTagIds.length > 0 && (
                  <button onClick={clearTagFilter} className="btn-secondary">
                    Clear Tags
                  </button>
                )}
              </div>
            </>
          ) : (
            <>
              <Receipt className="w-12 h-12 text-hone-300 mx-auto mb-4" />
              <h2 className="text-lg font-semibold mb-2">No Transactions</h2>
              <p className="text-hone-500">Import transactions to see them here.</p>
            </>
          )}
        </div>
      ) : (
        <>
          <div className="card">
            {loading && (
              <div className="absolute inset-0 bg-white/50 dark:bg-hone-900/50 flex items-center justify-center z-10">
                <RefreshCw className="w-6 h-6 text-hone-500 animate-spin" />
              </div>
            )}
            {/* Desktop header row */}
            <div className="hidden md:flex items-center gap-4 px-4 py-2 text-xs font-medium text-hone-400 uppercase tracking-wide border-b border-hone-100 dark:border-hone-700 bg-hone-50 dark:bg-hone-800/50">
              {/* Selection checkbox header */}
              {selectionMode && (
                <button
                  onClick={() => allOnPageSelected ? clearSelection() : selectAllOnPage()}
                  className="flex-shrink-0 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
                  title={allOnPageSelected ? "Deselect all" : "Select all on page"}
                >
                  {allOnPageSelected ? (
                    <SquareCheck className="w-5 h-5 text-hone-600 dark:text-hone-300" />
                  ) : (
                    <Square className="w-5 h-5" />
                  )}
                </button>
              )}
              <button
                onClick={() => toggleSort("date")}
                className="w-20 flex items-center gap-1 hover:text-hone-600 dark:hover:text-hone-200 transition-colors"
              >
                Date
                {sortField === "date" && (
                  sortOrder === "desc" ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />
                )}
              </button>
              <div className="flex-1">Description</div>
              <div className="w-[200px] text-center">Tags</div>
              <div className="w-3.5"></div>
              <button
                onClick={() => toggleSort("amount")}
                className="w-24 flex items-center justify-end gap-1 hover:text-hone-600 dark:hover:text-hone-200 transition-colors"
              >
                Amount
                {sortField === "amount" && (
                  sortOrder === "desc" ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />
                )}
              </button>
            </div>
            <div className="divide-y divide-hone-100 dark:divide-hone-700">
              {transactions.map((tx) => (
                <TransactionItem
                  key={tx.id}
                  transaction={tx}
                  onArchive={loadTransactions}
                  selectionMode={selectionMode}
                  selected={selectedTransactionIds.has(tx.id)}
                  onToggleSelect={toggleSelection}
                />
              ))}
            </div>
          </div>

          {/* Bulk action toolbar */}
          {selectedTransactionIds.size > 0 && (
            <div className="sticky bottom-4 mx-4 flex items-center justify-between gap-4 px-4 py-3 bg-hone-800 dark:bg-hone-700 text-white rounded-lg shadow-lg">
              <span className="text-sm font-medium">
                {selectedTransactionIds.size} selected
              </span>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => {
                    setBulkTagAction("add");
                    setShowBulkTagModal(true);
                  }}
                  className="px-3 py-1.5 text-sm bg-hone-600 hover:bg-hone-500 rounded-md flex items-center gap-1.5"
                >
                  <Tag className="w-4 h-4" />
                  Add Tags
                </button>
                <button
                  onClick={() => {
                    setBulkTagAction("remove");
                    setShowBulkTagModal(true);
                  }}
                  className="px-3 py-1.5 text-sm bg-hone-600 hover:bg-hone-500 rounded-md flex items-center gap-1.5"
                >
                  <X className="w-4 h-4" />
                  Remove Tags
                </button>
                <button
                  onClick={clearSelection}
                  className="px-3 py-1.5 text-sm text-hone-300 hover:text-white"
                >
                  Clear
                </button>
              </div>
            </div>
          )}

          {/* Pagination */}
          {total > limit && (
            <div className="flex items-center justify-between">
              <button
                onClick={() => setOffset(Math.max(0, offset - limit))}
                disabled={offset === 0}
                className="btn-secondary disabled:opacity-50"
              >
                Previous
              </button>
              <span className="text-sm text-hone-500">
                Showing {offset + 1}-{Math.min(offset + limit, total)} of {total}
              </span>
              <button
                onClick={() => setOffset(offset + limit)}
                disabled={offset + limit >= total}
                className="btn-secondary disabled:opacity-50"
              >
                Next
              </button>
            </div>
          )}
        </>
      )}

      {/* Bulk tag modal */}
      {showBulkTagModal && (
        <BulkTagModal
          transactionIds={Array.from(selectedTransactionIds)}
          action={bulkTagAction}
          onClose={() => setShowBulkTagModal(false)}
          onSuccess={() => {
            setShowBulkTagModal(false);
            clearSelection();
            exitSelectionMode();
            loadTransactions();
          }}
        />
      )}
    </div>
  );
}
