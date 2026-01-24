import { Archive, ChevronDown, Edit2, FileText, MapPin, RotateCcw, Tag, X } from "lucide-react";
import { useEffect, useState, useRef } from "react";
import { api } from "../../api";
import type {
  Entity,
  Location,
  Receipt,
  ReprocessResponse,
  TagWithPath,
  Transaction,
  TransactionSplitWithDetails,
  TransactionTag,
} from "../../types";
import { LocationTab, ReceiptsTab, SplitsTab, TagsTab } from "./tabs";

interface SplitsModalProps {
  transaction: Transaction;
  onClose: () => void;
  onArchive?: () => void;
  onTagsChange?: () => void;
}

type TabType = "splits" | "location" | "receipts" | "tags";

export function SplitsModal({ transaction, onClose, onArchive, onTagsChange }: SplitsModalProps) {
  // Shared data state
  const [splits, setSplits] = useState<TransactionSplitWithDetails[]>([]);
  const [entities, setEntities] = useState<Entity[]>([]);
  const [locations, setLocations] = useState<Location[]>([]);
  const [receipts, setReceipts] = useState<Receipt[]>([]);
  const [transactionTags, setTransactionTags] = useState<TransactionTag[]>([]);
  const [allTags, setAllTags] = useState<TagWithPath[]>([]);

  // UI state
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabType>("splits");

  // Header-specific state
  const [reprocessing, setReprocessing] = useState(false);
  const [reprocessResult, setReprocessResult] = useState<ReprocessResponse | null>(null);
  const [archiving, setArchiving] = useState(false);
  const [showRawData, setShowRawData] = useState(false);

  // Merchant name editing
  const [editingMerchant, setEditingMerchant] = useState(false);
  const [merchantName, setMerchantName] = useState(
    transaction.merchant_normalized || transaction.description
  );
  const [savingMerchant, setSavingMerchant] = useState(false);
  const [merchantUpdateCount, setMerchantUpdateCount] = useState<number | null>(null);
  const merchantInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadData();
  }, [transaction.id]);

  // Close on Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  // Focus merchant input when editing starts
  useEffect(() => {
    if (editingMerchant && merchantInputRef.current) {
      merchantInputRef.current.focus();
      merchantInputRef.current.select();
    }
  }, [editingMerchant]);

  const loadData = async () => {
    try {
      setLoading(true);
      const [splitsData, entitiesData, locationsData, receiptsData, tagsData, allTagsData] = await Promise.all([
        api.getTransactionSplits(transaction.id),
        api.getEntities(),
        api.getLocations(),
        api.getTransactionReceipts(transaction.id),
        api.getTransactionTags(transaction.id),
        api.getTagsTree(),
      ]);
      setSplits(splitsData);
      setEntities(entitiesData);
      setLocations(locationsData);
      setReceipts(receiptsData);
      setTransactionTags(tagsData);
      setAllTags(allTagsData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load data");
    } finally {
      setLoading(false);
    }
  };

  const handleReprocess = async () => {
    try {
      setReprocessing(true);
      setError(null);
      setReprocessResult(null);
      const result = await api.reprocessTransaction(transaction.id);
      setReprocessResult(result);
      if (!result.success && result.error) {
        setError(result.error);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to reprocess transaction");
    } finally {
      setReprocessing(false);
    }
  };

  const handleArchive = async () => {
    try {
      setArchiving(true);
      setError(null);
      await api.archiveTransaction(transaction.id);
      onClose();
      onArchive?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to archive transaction");
      setArchiving(false);
    }
  };

  const handleSaveMerchantName = async () => {
    const trimmed = merchantName.trim();
    if (!trimmed) {
      setError("Merchant name cannot be empty");
      return;
    }
    try {
      setSavingMerchant(true);
      setError(null);
      setMerchantUpdateCount(null);
      const result = await api.updateMerchantName(transaction.id, trimmed);
      setMerchantUpdateCount(result.updated_count);
      setEditingMerchant(false);
      transaction.merchant_normalized = trimmed;
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update merchant name");
    } finally {
      setSavingMerchant(false);
    }
  };

  const handleCancelMerchantEdit = () => {
    setMerchantName(transaction.merchant_normalized || transaction.description);
    setEditingMerchant(false);
    setMerchantUpdateCount(null);
  };

  const handleTagsChange = async () => {
    const updatedTags = await api.getTransactionTags(transaction.id);
    setTransactionTags(updatedTags);
    onTagsChange?.();
  };

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={onClose}
    >
      <div
        className="card w-full max-w-2xl mx-4 max-h-[90vh] flex flex-col animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="card-header">
          <div className="flex items-center justify-between mb-3">
            <div className="flex-1 min-w-0 mr-2">
              <h2 className="text-lg font-semibold">Transaction Details</h2>
              {/* Editable merchant name */}
              {editingMerchant ? (
                <div className="flex items-center gap-2 mt-1">
                  <input
                    ref={merchantInputRef}
                    type="text"
                    value={merchantName}
                    onChange={(e) => setMerchantName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSaveMerchantName();
                      if (e.key === "Escape") handleCancelMerchantEdit();
                    }}
                    className="flex-1 px-2 py-1 text-sm border border-hone-300 dark:border-hone-600 rounded focus:outline-none focus:ring-2 focus:ring-hone-500 bg-white dark:bg-hone-800"
                    placeholder="Merchant name"
                  />
                  <button
                    onClick={handleSaveMerchantName}
                    disabled={savingMerchant}
                    className="px-2 py-1 text-xs bg-hone-700 text-white rounded hover:bg-hone-800 disabled:opacity-50"
                  >
                    {savingMerchant ? "..." : "Save"}
                  </button>
                  <button
                    onClick={handleCancelMerchantEdit}
                    className="px-2 py-1 text-xs text-hone-500 hover:text-hone-700"
                  >
                    Cancel
                  </button>
                </div>
              ) : (
                <div className="group flex items-center gap-1">
                  <p className="text-sm text-hone-400 truncate" title={transaction.description}>
                    {merchantName}
                  </p>
                  <button
                    onClick={() => setEditingMerchant(true)}
                    className="p-0.5 text-hone-300 hover:text-hone-600 dark:hover:text-hone-200 opacity-0 group-hover:opacity-100 transition-opacity"
                    title="Edit merchant name (applies to all matching transactions)"
                  >
                    <Edit2 className="w-3 h-3" />
                  </button>
                </div>
              )}
              {/* Show update count feedback */}
              {merchantUpdateCount !== null && merchantUpdateCount > 1 && (
                <p className="text-xs text-savings mt-0.5">
                  Updated {merchantUpdateCount} transactions with same description
                </p>
              )}
              {/* Original description if different from normalized */}
              {!editingMerchant && transaction.merchant_normalized && transaction.merchant_normalized !== transaction.description && (
                <p className="text-xs text-hone-500 truncate">{transaction.description}</p>
              )}
              {/* Card member (from Amex extended format) */}
              {transaction.card_member && (
                <p className="text-xs text-hone-400 mt-0.5">
                  Card: {transaction.card_member}
                </p>
              )}
              {/* Payment method */}
              {transaction.payment_method && (
                <p className="text-xs text-hone-400 mt-0.5">
                  {transaction.payment_method === "apple_pay" && "Apple Pay"}
                  {transaction.payment_method === "google_pay" && "Google Pay"}
                  {transaction.payment_method === "physical_card" && "Physical Card"}
                  {transaction.payment_method === "online" && "Online"}
                  {transaction.payment_method === "recurring" && "Recurring"}
                </p>
              )}
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={handleReprocess}
                disabled={reprocessing}
                className="p-1.5 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200 hover:bg-hone-100 dark:hover:bg-hone-700 rounded transition-colors disabled:opacity-50"
                title="Re-run Ollama classification"
              >
                <RotateCcw className={`w-4 h-4 ${reprocessing ? "animate-spin" : ""}`} />
              </button>
              <button
                onClick={handleArchive}
                disabled={archiving}
                className="p-1.5 text-hone-400 hover:text-waste dark:hover:text-waste hover:bg-waste/10 rounded transition-colors disabled:opacity-50"
                title="Archive transaction"
              >
                <Archive className={`w-4 h-4 ${archiving ? "animate-pulse" : ""}`} />
              </button>
              <button
                onClick={onClose}
                className="p-1 text-hone-400 hover:text-hone-600"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
          </div>

          {/* Reprocess result message */}
          {reprocessResult && reprocessResult.success && (
            <div className="mb-3 p-2 bg-savings/10 text-savings text-sm rounded">
              Reprocessed: {reprocessResult.new_tag || "No tag assigned"}
              {reprocessResult.normalized_merchant && (
                <span className="text-hone-500 ml-2">
                  ({reprocessResult.normalized_merchant})
                </span>
              )}
            </div>
          )}

          {/* Raw transaction data (collapsible) */}
          <RawDataSection
            transaction={transaction}
            showRawData={showRawData}
            setShowRawData={setShowRawData}
          />

          {/* Tabs */}
          <div className="flex gap-1 border-b border-hone-100 dark:border-hone-700 -mb-4 -mx-4 px-4">
            <TabButton
              active={activeTab === "splits"}
              onClick={() => setActiveTab("splits")}
            >
              Splits
            </TabButton>
            <TabButton
              active={activeTab === "location"}
              onClick={() => setActiveTab("location")}
              icon={<MapPin className="w-3.5 h-3.5" />}
            >
              Location
            </TabButton>
            <TabButton
              active={activeTab === "tags"}
              onClick={() => setActiveTab("tags")}
              icon={<Tag className="w-3.5 h-3.5" />}
              badge={transactionTags.length > 0 ? transactionTags.length : undefined}
            >
              Tags
            </TabButton>
            <TabButton
              active={activeTab === "receipts"}
              onClick={() => setActiveTab("receipts")}
              icon={<FileText className="w-3.5 h-3.5" />}
              badge={receipts.length > 0 ? receipts.length : undefined}
            >
              Receipts
            </TabButton>
          </div>
        </div>

        {/* Tab Content */}
        <div className="card-body overflow-auto flex-1">
          {error && <div className="text-sm text-waste mb-4">{error}</div>}

          {activeTab === "splits" && (
            <SplitsTab
              transaction={transaction}
              splits={splits}
              entities={entities}
              loading={loading}
              onSplitsChange={loadData}
              onError={setError}
            />
          )}

          {activeTab === "location" && (
            <LocationTab
              transaction={transaction}
              locations={locations}
              loading={loading}
              onLocationsChange={setLocations}
              onError={setError}
            />
          )}

          {activeTab === "tags" && (
            <TagsTab
              transaction={transaction}
              transactionTags={transactionTags}
              allTags={allTags}
              loading={loading}
              onTagsChange={handleTagsChange}
              onError={setError}
            />
          )}

          {activeTab === "receipts" && (
            <ReceiptsTab
              transaction={transaction}
              receipts={receipts}
              loading={loading}
              onReceiptsChange={loadData}
              onError={setError}
            />
          )}
        </div>

        {/* Footer */}
        <div className="card-body border-t border-hone-100 flex justify-end">
          <button onClick={onClose} className="btn-secondary">
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

// Tab button component
interface TabButtonProps {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  icon?: React.ReactNode;
  badge?: number;
}

function TabButton({ active, onClick, children, icon, badge }: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      className={`px-3 py-2 text-sm font-medium border-b-2 transition-colors flex items-center gap-1 ${
        active
          ? "border-hone-700 text-hone-900 dark:text-hone-100"
          : "border-transparent text-hone-500 hover:text-hone-700 dark:text-hone-400 dark:hover:text-hone-200"
      }`}
    >
      {icon}
      {children}
      {badge !== undefined && (
        <span className="ml-1 px-1.5 py-0.5 text-xs bg-hone-200 dark:bg-hone-700 rounded-full">
          {badge}
        </span>
      )}
    </button>
  );
}

// Raw data section component
interface RawDataSectionProps {
  transaction: Transaction;
  showRawData: boolean;
  setShowRawData: (show: boolean) => void;
}

function RawDataSection({ transaction, showRawData, setShowRawData }: RawDataSectionProps) {
  return (
    <div className="mb-3">
      <button
        onClick={() => setShowRawData(!showRawData)}
        className="flex items-center gap-1 text-xs text-hone-400 hover:text-hone-600 dark:hover:text-hone-300"
      >
        <ChevronDown className={`w-3 h-3 transition-transform ${showRawData ? "" : "-rotate-90"}`} />
        Raw Data
        {transaction.import_format && (
          <span className="ml-1 px-1.5 py-0.5 bg-hone-200 dark:bg-hone-700 rounded text-hone-600 dark:text-hone-300">
            {transaction.import_format}
          </span>
        )}
      </button>
      {showRawData && (
        <div className="mt-2 p-2 bg-hone-50 dark:bg-hone-800 rounded text-xs font-mono space-y-1">
          {/* Show original import data if available */}
          {transaction.original_data && (
            <div className="mb-3 pb-2 border-b border-hone-200 dark:border-hone-700">
              <div className="text-hone-500 mb-1 font-semibold">Original Import Data:</div>
              {(() => {
                try {
                  const parsed = JSON.parse(transaction.original_data);
                  return Object.entries(parsed).map(([key, value]) => (
                    <div key={key}>
                      <span className="text-hone-500">{key}:</span>{" "}
                      <span className="text-hone-700 dark:text-hone-300 break-all">
                        {String(value)}
                      </span>
                    </div>
                  ));
                } catch {
                  return (
                    <span className="text-hone-700 dark:text-hone-300 break-all">
                      {transaction.original_data}
                    </span>
                  );
                }
              })()}
            </div>
          )}
          {/* Processed transaction data */}
          <div className="text-hone-500 mb-1 font-semibold">Processed Data:</div>
          <RawDataField label="ID" value={transaction.id} />
          <RawDataField label="Account ID" value={transaction.account_id} />
          <RawDataField label="Date" value={transaction.date} />
          <RawDataField label="Description" value={transaction.description} />
          <RawDataField label="Amount" value={`$${transaction.amount.toFixed(2)}`} />
          {transaction.category && <RawDataField label="Category" value={transaction.category} />}
          {transaction.merchant_normalized && <RawDataField label="Merchant Normalized" value={transaction.merchant_normalized} />}
          <RawDataField label="Import Hash" value={transaction.import_hash} />
          {transaction.purchase_location_id && <RawDataField label="Purchase Location ID" value={transaction.purchase_location_id} />}
          {transaction.vendor_location_id && <RawDataField label="Vendor Location ID" value={transaction.vendor_location_id} />}
          {transaction.trip_id && <RawDataField label="Trip ID" value={transaction.trip_id} />}
          <RawDataField label="Source" value={transaction.source} />
          {transaction.expected_amount !== null && <RawDataField label="Expected Amount" value={`$${transaction.expected_amount.toFixed(2)}`} />}
          <RawDataField label="Archived" value={transaction.archived ? "Yes" : "No"} />
          <RawDataField label="Created At" value={transaction.created_at} />
        </div>
      )}
    </div>
  );
}

function RawDataField({ label, value }: { label: string; value: string | number }) {
  return (
    <div>
      <span className="text-hone-500">{label}:</span>{" "}
      <span className="text-hone-700 dark:text-hone-300 break-all">{value}</span>
    </div>
  );
}
