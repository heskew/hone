import { Camera, FileText, Link2, RefreshCw, Trash2, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import type { Receipt, ReceiptStatus, Transaction } from "../../types";

const STATUS_LABELS: Record<ReceiptStatus, string> = {
  pending: "Pending",
  matched: "Matched",
  manual_review: "Review",
  orphaned: "Orphaned",
};

const STATUS_COLORS: Record<ReceiptStatus, string> = {
  pending: "bg-attention/10 text-attention",
  matched: "bg-savings/10 text-savings",
  manual_review: "bg-hone-200 text-hone-600",
  orphaned: "bg-waste/10 text-waste",
};

export function ReceiptsPage() {
  const [receipts, setReceipts] = useState<Receipt[]>([]);
  const [statusFilter, setStatusFilter] = useState<ReceiptStatus | "all">("pending");
  const [loading, setLoading] = useState(true);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [linkingReceipt, setLinkingReceipt] = useState<Receipt | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadReceipts();
  }, [statusFilter]);

  const loadReceipts = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await api.getReceipts(
        statusFilter === "all" ? undefined : statusFilter
      );
      setReceipts(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load receipts");
    } finally {
      setLoading(false);
    }
  };

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    try {
      setUploading(true);
      setError(null);
      await api.uploadPendingReceipt(file);
      await loadReceipts();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to upload receipt");
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    }
  };

  const handleDelete = async (id: number) => {
    if (!confirm("Delete this receipt?")) return;

    try {
      await api.deleteReceipt(id);
      setReceipts(receipts.filter((r) => r.id !== id));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete receipt");
    }
  };

  const handleParse = async (id: number) => {
    try {
      setError(null);
      await api.parseReceipt(id);
      await loadReceipts();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to parse receipt");
    }
  };

  const handleMarkOrphaned = async (id: number) => {
    try {
      await api.updateReceiptStatus(id, "orphaned");
      await loadReceipts();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to update receipt status"
      );
    }
  };

  return (
    <div className="space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Receipts</h1>
        <div className="flex items-center gap-2">
          <input
            ref={fileInputRef}
            type="file"
            accept="image/*"
            onChange={handleFileSelect}
            className="hidden"
          />
          <button
            onClick={() => fileInputRef.current?.click()}
            disabled={uploading}
            className="btn-primary flex items-center gap-2"
          >
            {uploading ? (
              <RefreshCw className="w-4 h-4 animate-spin" />
            ) : (
              <Upload className="w-4 h-4" />
            )}
            Upload Receipt
          </button>
        </div>
      </div>

      {/* Status filter tabs */}
      <div className="flex gap-1 flex-wrap">
        {(["all", "pending", "matched", "manual_review", "orphaned"] as const).map(
          (status) => (
            <button
              key={status}
              onClick={() => setStatusFilter(status)}
              className={`px-3 py-1.5 text-sm rounded-lg transition-colors ${
                statusFilter === status
                  ? "bg-hone-700 text-white"
                  : "bg-hone-100 text-hone-600 hover:bg-hone-200"
              }`}
            >
              {status === "all"
                ? "All"
                : status === "manual_review"
                  ? "Manual Review"
                  : status.charAt(0).toUpperCase() + status.slice(1)}
            </button>
          )
        )}
      </div>

      {error && (
        <div className="p-3 bg-waste/10 text-waste rounded-lg text-sm">
          {error}
        </div>
      )}

      {loading ? (
        <div className="card p-8 text-center">
          <RefreshCw className="w-8 h-8 text-hone-300 mx-auto mb-4 animate-spin" />
          <p className="text-hone-500">Loading receipts...</p>
        </div>
      ) : receipts.length === 0 ? (
        <div className="card p-8 text-center">
          <Camera className="w-12 h-12 text-hone-300 mx-auto mb-4" />
          <h2 className="text-lg font-semibold mb-2">No Receipts</h2>
          <p className="text-hone-500">
            Upload receipts to track and match them with transactions.
          </p>
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {receipts.map((receipt) => (
            <ReceiptCard
              key={receipt.id}
              receipt={receipt}
              onDelete={() => handleDelete(receipt.id)}
              onParse={() => handleParse(receipt.id)}
              onLink={() => setLinkingReceipt(receipt)}
              onMarkOrphaned={() => handleMarkOrphaned(receipt.id)}
            />
          ))}
        </div>
      )}

      {linkingReceipt && (
        <LinkReceiptModal
          receipt={linkingReceipt}
          onClose={() => setLinkingReceipt(null)}
          onLinked={() => {
            setLinkingReceipt(null);
            loadReceipts();
          }}
        />
      )}
    </div>
  );
}

interface ReceiptCardProps {
  receipt: Receipt;
  onDelete: () => void;
  onParse: () => void;
  onLink: () => void;
  onMarkOrphaned: () => void;
}

function ReceiptCard({
  receipt,
  onDelete,
  onParse,
  onLink,
  onMarkOrphaned,
}: ReceiptCardProps) {
  const hasImage = !!receipt.image_path;
  const hasParsedData = !!receipt.parsed_json;
  const isPending = receipt.status === "pending";
  const needsReview = receipt.status === "manual_review";

  return (
    <div className="card overflow-hidden">
      {/* Image placeholder or thumbnail */}
      <div className="h-32 bg-hone-100 flex items-center justify-center">
        {hasImage ? (
          <img
            src={`/api/receipts/${receipt.id}/image`}
            alt="Receipt"
            className="h-full w-full object-cover"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = "none";
            }}
          />
        ) : (
          <FileText className="w-12 h-12 text-hone-300" />
        )}
      </div>

      <div className="p-4 space-y-3">
        {/* Status badge */}
        <div className="flex items-center justify-between">
          <span
            className={`px-2 py-0.5 text-xs font-medium rounded ${STATUS_COLORS[receipt.status]}`}
          >
            {STATUS_LABELS[receipt.status]}
          </span>
          <span className="text-xs text-hone-400">
            {new Date(receipt.created_at).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "2-digit" })}
          </span>
        </div>

        {/* Parsed info */}
        {receipt.receipt_merchant && (
          <p className="font-medium truncate">{receipt.receipt_merchant}</p>
        )}
        {receipt.receipt_total !== null && (
          <p className="text-lg font-semibold">
            ${receipt.receipt_total.toFixed(2)}
          </p>
        )}
        {receipt.receipt_date && (
          <p className="text-sm text-hone-500">
            Receipt date:{" "}
            {(() => {
              const [y, m, d] = receipt.receipt_date.split("-").map(Number);
              return new Date(y, m - 1, d, 12, 0, 0).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "2-digit" });
            })()}
          </p>
        )}

        {/* Actions */}
        <div className="flex gap-2 pt-2 border-t border-hone-100">
          {!hasParsedData && hasImage && (
            <button
              onClick={onParse}
              className="flex-1 btn-secondary text-sm py-1.5"
              title="Parse receipt with AI"
            >
              Parse
            </button>
          )}
          {(isPending || needsReview) && (
            <button
              onClick={onLink}
              className="flex-1 btn-primary text-sm py-1.5 flex items-center justify-center gap-1"
            >
              <Link2 className="w-3.5 h-3.5" />
              Link
            </button>
          )}
          {isPending && (
            <button
              onClick={onMarkOrphaned}
              className="btn-secondary text-sm py-1.5"
              title="Mark as orphaned (no matching transaction)"
            >
              Orphan
            </button>
          )}
          <button
            onClick={onDelete}
            className="p-1.5 text-hone-400 hover:text-waste hover:bg-waste/10 rounded"
            title="Delete receipt"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>
  );
}

interface LinkReceiptModalProps {
  receipt: Receipt;
  onClose: () => void;
  onLinked: () => void;
}

function LinkReceiptModal({ receipt, onClose, onLinked }: LinkReceiptModalProps) {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [linking, setLinking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  useEffect(() => {
    loadTransactions();
  }, []);

  const loadTransactions = async () => {
    try {
      setLoading(true);
      // Load recent transactions to match against
      const result = await api.getTransactions({ limit: 100 });
      setTransactions(result.transactions);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load transactions"
      );
    } finally {
      setLoading(false);
    }
  };

  const handleLink = async (transactionId: number) => {
    try {
      setLinking(true);
      setError(null);
      await api.linkReceipt(receipt.id, transactionId);
      onLinked();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to link receipt");
      setLinking(false);
    }
  };

  // Filter transactions based on search and receipt info
  const filteredTransactions = transactions.filter((tx) => {
    if (!search) return true;
    const searchLower = search.toLowerCase();
    return (
      tx.description.toLowerCase().includes(searchLower) ||
      tx.merchant_normalized?.toLowerCase().includes(searchLower)
    );
  });

  // Sort by relevance - try to match amount and date
  const sortedTransactions = [...filteredTransactions].sort((a, b) => {
    let scoreA = 0;
    let scoreB = 0;

    // Boost if amount matches
    if (receipt.receipt_total !== null) {
      if (Math.abs(Math.abs(a.amount) - receipt.receipt_total) < 0.01)
        scoreA += 100;
      if (Math.abs(Math.abs(b.amount) - receipt.receipt_total) < 0.01)
        scoreB += 100;
    }

    // Boost if date matches (receipt_date is already YYYY-MM-DD format, compare directly)
    if (receipt.receipt_date) {
      if (a.date === receipt.receipt_date) scoreA += 50;
      if (b.date === receipt.receipt_date) scoreB += 50;
    }

    // Boost if merchant matches
    if (receipt.receipt_merchant) {
      const merchantLower = receipt.receipt_merchant.toLowerCase();
      if (a.description.toLowerCase().includes(merchantLower)) scoreA += 30;
      if (b.description.toLowerCase().includes(merchantLower)) scoreB += 30;
    }

    return scoreB - scoreA;
  });

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-lg mx-4 max-h-[80vh] flex flex-col animate-slide-up">
        <div className="card-header">
          <h2 className="text-lg font-semibold">Link Receipt to Transaction</h2>
          {receipt.receipt_merchant && (
            <p className="text-sm text-hone-400">
              {receipt.receipt_merchant}
              {receipt.receipt_total !== null &&
                ` - $${receipt.receipt_total.toFixed(2)}`}
            </p>
          )}
        </div>

        <div className="p-4 border-b border-hone-100">
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search transactions..."
            className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
          />
        </div>

        <div className="flex-1 overflow-auto">
          {loading ? (
            <div className="p-8 text-center text-hone-500">Loading...</div>
          ) : error ? (
            <div className="p-4 text-waste text-sm">{error}</div>
          ) : sortedTransactions.length === 0 ? (
            <div className="p-8 text-center text-hone-500">
              No transactions found
            </div>
          ) : (
            <div className="divide-y divide-hone-100">
              {sortedTransactions.slice(0, 20).map((tx) => {
                const amountMatch =
                  receipt.receipt_total !== null &&
                  Math.abs(Math.abs(tx.amount) - receipt.receipt_total) < 0.01;
                const dateMatch =
                  receipt.receipt_date &&
                  tx.date === receipt.receipt_date.slice(0, 10);

                return (
                  <button
                    key={tx.id}
                    onClick={() => handleLink(tx.id)}
                    disabled={linking}
                    className="w-full p-3 text-left hover:bg-hone-100 dark:hover:bg-hone-700 transition-colors disabled:opacity-50"
                  >
                    <div className="flex justify-between items-start">
                      <div className="flex-1 min-w-0">
                        <p className="font-medium truncate">
                          {tx.description}
                        </p>
                        <p className="text-sm text-hone-400">
                          {(() => {
                            const [y, m, d] = tx.date.split("-").map(Number);
                            return new Date(y, m - 1, d, 12, 0, 0).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "2-digit" });
                          })()}
                          {(amountMatch || dateMatch) && (
                            <span className="ml-2">
                              {amountMatch && (
                                <span className="text-savings text-xs">
                                  Amount match
                                </span>
                              )}
                              {amountMatch && dateMatch && " "}
                              {dateMatch && (
                                <span className="text-savings text-xs">
                                  Date match
                                </span>
                              )}
                            </span>
                          )}
                        </p>
                      </div>
                      <span className="font-medium">
                        ${Math.abs(tx.amount).toFixed(2)}
                      </span>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <div className="card-body border-t border-hone-100 flex justify-end">
          <button onClick={onClose} className="btn-secondary" disabled={linking}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
