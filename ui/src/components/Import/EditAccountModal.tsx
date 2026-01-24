import { X } from "lucide-react";
import { useState } from "react";
import { api } from "../../api";
import type { Account, Bank } from "../../types";

interface EditAccountModalProps {
  account: Account;
  onClose: () => void;
  onUpdated: () => void;
}

export function EditAccountModal({ account, onClose, onUpdated }: EditAccountModalProps) {
  const [name, setName] = useState(account.name);
  const [bank, setBank] = useState<Bank>(account.bank);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Alphabetically sorted bank options
  const bankOptions: { value: Bank; label: string }[] = [
    { value: "amex", label: "American Express" },
    { value: "bofa", label: "Bank of America" },
    { value: "capitalone", label: "Capital One" },
    { value: "chase", label: "Chase" },
  ];

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    try {
      setSaving(true);
      setError(null);
      await api.updateAccount(account.id, { name: name.trim(), bank });
      onUpdated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update account");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold">Edit Account</h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="card-body space-y-4">
            <div>
              <label className="block text-sm font-medium text-hone-700 dark:text-hone-300 mb-1">
                Account Name
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g., Chase Checking, BECU Savings"
                className="w-full px-3 py-2 border border-hone-200 dark:border-hone-600 rounded-lg bg-white dark:bg-hone-800 text-hone-900 dark:text-hone-100 placeholder:text-hone-400 focus:outline-none focus:ring-2 focus:ring-hone-500"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-hone-700 dark:text-hone-300 mb-1">
                Bank Format
              </label>
              <select
                value={bank}
                onChange={(e) => setBank(e.target.value as Bank)}
                className="w-full px-3 py-2 border border-hone-200 dark:border-hone-600 rounded-lg bg-white dark:bg-hone-800 text-hone-900 dark:text-hone-100 focus:outline-none focus:ring-2 focus:ring-hone-500"
              >
                {bankOptions.map((opt) => (
                  <option key={opt.value} value={opt.value} className="bg-white dark:bg-hone-800 text-hone-900 dark:text-hone-100">
                    {opt.label}
                  </option>
                ))}
              </select>
              <p className="mt-1 text-sm text-hone-500 dark:text-hone-400">
                Select the bank that matches your CSV format
              </p>
            </div>
            {error && (
              <div className="text-sm text-waste">{error}</div>
            )}
          </div>
          <div className="card-body border-t border-hone-100 dark:border-hone-700 flex justify-end gap-2">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button
              type="submit"
              disabled={!name.trim() || saving}
              className="btn-primary disabled:opacity-50"
            >
              {saving ? "Saving..." : "Save Changes"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
