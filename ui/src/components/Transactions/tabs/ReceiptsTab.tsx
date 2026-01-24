import { FileText, RefreshCw, Upload } from "lucide-react";
import { useRef, useState } from "react";
import { api } from "../../../api";
import type { Receipt, Transaction } from "../../../types";

interface ReceiptsTabProps {
  transaction: Transaction;
  receipts: Receipt[];
  loading: boolean;
  onReceiptsChange: () => void;
  onError: (error: string | null) => void;
}

export function ReceiptsTab({
  transaction,
  receipts,
  loading,
  onReceiptsChange,
  onError,
}: ReceiptsTabProps) {
  const [uploading, setUploading] = useState(false);
  const [parsing, setParsing] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleUploadReceipt = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    try {
      setUploading(true);
      onError(null);
      await api.uploadReceipt(transaction.id, file);
      onReceiptsChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to upload receipt");
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    }
  };

  const handleUnlinkReceipt = async (receiptId: number) => {
    try {
      onError(null);
      await api.unlinkReceipt(receiptId);
      onReceiptsChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to unlink receipt");
    }
  };

  const handleParseReceipt = async (receiptId: number) => {
    try {
      setParsing(true);
      onError(null);
      await api.parseReceipt(receiptId);
      onReceiptsChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to parse receipt");
    } finally {
      setParsing(false);
    }
  };

  if (loading) {
    return <p className="text-hone-500 text-center py-4">Loading...</p>;
  }

  return (
    <div className="space-y-4">
      {/* Upload button */}
      <div>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*,.pdf"
          onChange={handleUploadReceipt}
          className="hidden"
        />
        <button
          onClick={() => fileInputRef.current?.click()}
          disabled={uploading}
          className="btn-secondary w-full flex items-center justify-center gap-2"
        >
          {uploading ? (
            <>
              <RefreshCw className="w-4 h-4 animate-spin" />
              Uploading...
            </>
          ) : (
            <>
              <Upload className="w-4 h-4" />
              Upload Receipt
            </>
          )}
        </button>
      </div>

      {/* Receipts list */}
      {receipts.length === 0 ? (
        <div className="text-center py-8">
          <FileText className="w-12 h-12 text-hone-300 mx-auto mb-3" />
          <p className="text-hone-500">No receipts attached</p>
          <p className="text-sm text-hone-400 mt-1">
            Upload a receipt image to attach it to this transaction
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {receipts.map((receipt) => (
            <div
              key={receipt.id}
              className="p-3 bg-hone-50 dark:bg-hone-800 rounded-lg"
            >
              <div className="flex items-start justify-between mb-2">
                <div className="flex items-center gap-2">
                  <FileText className="w-5 h-5 text-hone-500" />
                  <div>
                    <p className="font-medium text-sm">
                      {receipt.receipt_merchant || "Receipt"}
                    </p>
                    <p className="text-xs text-hone-400">
                      {receipt.receipt_date || new Date(receipt.created_at).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "2-digit" })}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  <span
                    className={`text-xs px-2 py-0.5 rounded ${
                      receipt.status === "matched"
                        ? "bg-savings/20 text-savings"
                        : receipt.status === "pending"
                          ? "bg-attention/20 text-attention"
                          : "bg-hone-200 text-hone-600"
                    }`}
                  >
                    {receipt.status}
                  </span>
                </div>
              </div>

              {/* Parsed data */}
              {receipt.receipt_total && (
                <div className="text-sm mb-2">
                  <span className="text-hone-500">Total: </span>
                  <span className="font-medium">
                    ${receipt.receipt_total.toFixed(2)}
                  </span>
                </div>
              )}

              {/* Actions */}
              <div className="flex gap-2 mt-2">
                {receipt.image_path && (
                  <a
                    href={`/api/receipts/${receipt.id}/image`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-xs text-hone-600 hover:text-hone-800 underline"
                  >
                    View Image
                  </a>
                )}
                {!receipt.parsed_at && (
                  <button
                    onClick={() => handleParseReceipt(receipt.id)}
                    disabled={parsing}
                    className="text-xs text-hone-600 hover:text-hone-800 underline"
                  >
                    {parsing ? "Parsing..." : "Parse with AI"}
                  </button>
                )}
                <button
                  onClick={() => handleUnlinkReceipt(receipt.id)}
                  className="text-xs text-waste hover:text-waste/80 underline ml-auto"
                >
                  Unlink
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
