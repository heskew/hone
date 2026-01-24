import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api";
import type {
  Entity,
  SplitType,
  Transaction,
  TransactionSplitWithDetails,
} from "../../../types";

const SPLIT_TYPES: { value: SplitType; label: string }[] = [
  { value: "item", label: "Item" },
  { value: "tax", label: "Tax" },
  { value: "tip", label: "Tip" },
  { value: "fee", label: "Fee" },
  { value: "discount", label: "Discount" },
  { value: "rewards", label: "Rewards" },
];

interface SplitsTabProps {
  transaction: Transaction;
  splits: TransactionSplitWithDetails[];
  entities: Entity[];
  loading: boolean;
  onSplitsChange: () => void;
  onError: (error: string | null) => void;
}

export function SplitsTab({
  transaction,
  splits,
  entities,
  loading,
  onSplitsChange,
  onError,
}: SplitsTabProps) {
  const [saving, setSaving] = useState(false);
  const [newSplit, setNewSplit] = useState({
    amount: "",
    description: "",
    split_type: "item" as SplitType,
    entity_id: null as number | null,
    purchaser_id: null as number | null,
  });

  const totalSplits = splits.reduce((sum, s) => sum + s.amount, 0);
  const remaining = Math.abs(transaction.amount) - totalSplits;

  const handleAddSplit = async (e: React.FormEvent) => {
    e.preventDefault();
    const amount = parseFloat(newSplit.amount);
    if (isNaN(amount) || amount === 0) {
      onError("Please enter a valid amount");
      return;
    }

    try {
      setSaving(true);
      onError(null);
      await api.createSplit(transaction.id, {
        amount,
        description: newSplit.description || undefined,
        split_type: newSplit.split_type,
        entity_id: newSplit.entity_id || undefined,
        purchaser_id: newSplit.purchaser_id || undefined,
      });

      setNewSplit({
        amount: "",
        description: "",
        split_type: "item",
        entity_id: null,
        purchaser_id: null,
      });
      onSplitsChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to add split");
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteSplit = async (splitId: number) => {
    try {
      onError(null);
      await api.deleteSplit(splitId);
      onSplitsChange();
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to delete split");
    }
  };

  return (
    <>
      {/* Summary */}
      <div className="mb-4 p-3 bg-hone-50 dark:bg-hone-800 rounded-lg">
        <div className="flex justify-between text-sm">
          <span>Transaction Total:</span>
          <span className="font-medium">
            ${Math.abs(transaction.amount).toFixed(2)}
          </span>
        </div>
        <div className="flex justify-between text-sm">
          <span>Allocated:</span>
          <span className="font-medium">${totalSplits.toFixed(2)}</span>
        </div>
        <div className="flex justify-between text-sm border-t border-hone-200 pt-1 mt-1">
          <span>Remaining:</span>
          <span
            className={`font-medium ${remaining > 0.01 ? "text-attention" : "text-savings"}`}
          >
            ${remaining.toFixed(2)}
          </span>
        </div>
      </div>

      {loading ? (
        <p className="text-hone-500 text-center py-4">Loading splits...</p>
      ) : (
        <>
          {/* Existing splits */}
          {splits.length > 0 && (
            <div className="space-y-2 mb-4">
              <h3 className="text-sm font-medium text-hone-600">
                Current Splits
              </h3>
              {splits.map((split) => (
                <div
                  key={split.id}
                  className="flex items-center justify-between p-2 bg-hone-50 dark:bg-hone-800 rounded"
                >
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-xs px-1.5 py-0.5 bg-hone-200 rounded">
                        {split.split_type}
                      </span>
                      <span className="font-medium">
                        ${split.amount.toFixed(2)}
                      </span>
                    </div>
                    {split.description && (
                      <p className="text-sm text-hone-500">
                        {split.description}
                      </p>
                    )}
                    {(split.entity_name || split.purchaser_name) && (
                      <p className="text-xs text-hone-400">
                        {split.entity_name && `For: ${split.entity_name}`}
                        {split.entity_name && split.purchaser_name && " | "}
                        {split.purchaser_name &&
                          `By: ${split.purchaser_name}`}
                      </p>
                    )}
                  </div>
                  <button
                    onClick={() => handleDeleteSplit(split.id)}
                    className="p-1 text-hone-400 hover:text-waste"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Add new split form */}
          <form onSubmit={handleAddSplit} className="space-y-3">
            <h3 className="text-sm font-medium text-hone-600">Add Split</h3>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs text-hone-500 mb-1">
                  Amount
                </label>
                <input
                  type="number"
                  step="0.01"
                  value={newSplit.amount}
                  onChange={(e) =>
                    setNewSplit({ ...newSplit, amount: e.target.value })
                  }
                  placeholder={remaining > 0 ? remaining.toFixed(2) : "0.00"}
                  className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-hone-500 mb-1">
                  Type
                </label>
                <select
                  value={newSplit.split_type}
                  onChange={(e) =>
                    setNewSplit({
                      ...newSplit,
                      split_type: e.target.value as SplitType,
                    })
                  }
                  className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
                >
                  {SPLIT_TYPES.map((type) => (
                    <option key={type.value} value={type.value}>
                      {type.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div>
              <label className="block text-xs text-hone-500 mb-1">
                Description
              </label>
              <input
                type="text"
                value={newSplit.description}
                onChange={(e) =>
                  setNewSplit({ ...newSplit, description: e.target.value })
                }
                placeholder="e.g., Coffee, Groceries, Gas"
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
              />
            </div>

            {entities.length > 0 && (
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-hone-500 mb-1">
                    For (Entity)
                  </label>
                  <select
                    value={newSplit.entity_id || ""}
                    onChange={(e) =>
                      setNewSplit({
                        ...newSplit,
                        entity_id: e.target.value
                          ? parseInt(e.target.value)
                          : null,
                      })
                    }
                    className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
                  >
                    <option value="">None</option>
                    {entities.map((entity) => (
                      <option key={entity.id} value={entity.id}>
                        {entity.name} ({entity.entity_type})
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label className="block text-xs text-hone-500 mb-1">
                    Purchased By
                  </label>
                  <select
                    value={newSplit.purchaser_id || ""}
                    onChange={(e) =>
                      setNewSplit({
                        ...newSplit,
                        purchaser_id: e.target.value
                          ? parseInt(e.target.value)
                          : null,
                      })
                    }
                    className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
                  >
                    <option value="">None</option>
                    {entities
                      .filter((e) => e.entity_type === "person")
                      .map((entity) => (
                        <option key={entity.id} value={entity.id}>
                          {entity.name}
                        </option>
                      ))}
                  </select>
                </div>
              </div>
            )}

            <button
              type="submit"
              disabled={saving || !newSplit.amount}
              className="btn-primary w-full flex items-center justify-center gap-2 disabled:opacity-50"
            >
              <Plus className="w-4 h-4" />
              {saving ? "Adding..." : "Add Split"}
            </button>
          </form>
        </>
      )}
    </>
  );
}
