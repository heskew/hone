import { AlertTriangle, Check, ChevronDown, Pencil, Plus, RefreshCw, Settings, Trash2, Upload, User } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api";
import type { Account, Entity } from "../../types";
import { NewAccountModal } from "./NewAccountModal";
import { EditAccountModal } from "./EditAccountModal";

interface ImportViewProps {
  accounts: Account[];
  onAccountCreated: (account: Account) => void;
  onImportComplete: () => void;
  onAccountUpdated: () => void;
}

export function ImportView({ accounts, onAccountCreated, onImportComplete, onAccountUpdated }: ImportViewProps) {
  const [file, setFile] = useState<File | null>(null);
  const [selectedAccountId, setSelectedAccountId] = useState<number | null>(null);
  const [showNewAccountModal, setShowNewAccountModal] = useState(false);
  const [importing, setImporting] = useState(false);
  const [importPhase, setImportPhase] = useState<string>("Starting...");
  const [importResult, setImportResult] = useState<{
    success: boolean;
    message: string;
  } | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [entities, setEntities] = useState<Entity[]>([]);
  const [editingAccountEntity, setEditingAccountEntity] = useState<number | null>(null);
  const [editingAccount, setEditingAccount] = useState<Account | null>(null);
  const [deletingAccountId, setDeletingAccountId] = useState<number | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  // Model selection state
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [defaultModel, setDefaultModel] = useState<string>("");
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [loadingModels, setLoadingModels] = useState(false);

  // Load entities (people) on mount
  useEffect(() => {
    api.getEntities({ entity_type: "person" }).then(setEntities).catch(console.error);
  }, []);

  // Load available models on mount
  useEffect(() => {
    setLoadingModels(true);
    api.getExploreModels()
      .then((response) => {
        setAvailableModels(response.models);
        setDefaultModel(response.default_model);
        setSelectedModel(response.default_model);
      })
      .catch((err) => {
        console.error("Failed to load models:", err);
        // Don't show error - model selection is optional
      })
      .finally(() => setLoadingModels(false));
  }, []);

  const handleUpdateAccountEntity = async (accountId: number, entityId: number | null) => {
    try {
      await api.updateAccountEntity(accountId, entityId);
      setEditingAccountEntity(null);
      onAccountUpdated();
    } catch (err) {
      console.error("Failed to update account owner:", err);
    }
  };

  const getEntityName = (entityId: number | null) => {
    if (!entityId) return null;
    return entities.find((e) => e.id === entityId)?.name || null;
  };

  const handleDeleteAccount = async (accountId: number) => {
    try {
      setDeleteError(null);
      await api.deleteAccount(accountId);
      setDeletingAccountId(null);
      if (selectedAccountId === accountId) {
        setSelectedAccountId(null);
      }
      onAccountUpdated();
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : "Failed to delete account");
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const droppedFile = e.dataTransfer.files[0];
    if (droppedFile && droppedFile.name.endsWith(".csv")) {
      setFile(droppedFile);
      setImportResult(null);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = e.target.files?.[0];
    if (selectedFile) {
      setFile(selectedFile);
      setImportResult(null);
    }
  };

  const handleImport = async () => {
    if (!file || !selectedAccountId) return;

    try {
      setImporting(true);
      setImportResult(null);
      setImportPhase("Importing transactions...");

      // Use model override if different from default
      const modelToUse = selectedModel !== defaultModel ? selectedModel : undefined;
      const result = await api.importCsv(file, selectedAccountId, modelToUse);

      // Build success message - note that AI processing happens in background now
      let message = `Imported ${result.imported} transactions`;
      if (result.skipped > 0) {
        message += ` (${result.skipped} duplicates skipped)`;
      }
      if (result.imported > 0) {
        message += `. AI processing started in background.`;
      }

      setImportResult({
        success: true,
        message,
      });
      // Clear file after successful import
      setFile(null);
      // Navigate to import history to show progress
      setTimeout(() => {
        onImportComplete();
        // Navigate to history page with this session open
        window.location.hash = `#/history/${result.import_session_id}`;
      }, 1500);
    } catch (err) {
      setImportResult({
        success: false,
        message: err instanceof Error ? err.message : "Import failed",
      });
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="space-y-6 animate-fade-in">
      <h1 className="text-2xl font-bold">Import Transactions</h1>

      {/* File Drop Zone */}
      <div
        onDrop={handleDrop}
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        className={`card p-8 border-2 border-dashed transition-colors ${
          dragOver ? "border-hone-500 bg-hone-50 dark:bg-hone-800" : "border-hone-200 dark:border-hone-700"
        }`}
      >
        <div className="text-center">
          <Upload className={`w-12 h-12 mx-auto mb-4 ${dragOver ? "text-hone-500" : "text-hone-300"}`} />
          {file ? (
            <div>
              <p className="font-medium text-hone-900 dark:text-hone-100">{file.name}</p>
              <p className="text-sm text-hone-500">{(file.size / 1024).toFixed(1)} KB</p>
              <button
                onClick={() => setFile(null)}
                className="mt-2 text-sm text-waste hover:underline"
              >
                Remove
              </button>
            </div>
          ) : (
            <div>
              <p className="text-hone-600 mb-2">Drag and drop a CSV file here, or</p>
              <label className="btn-secondary cursor-pointer">
                <input
                  type="file"
                  accept=".csv"
                  onChange={handleFileSelect}
                  className="hidden"
                />
                Browse files
              </label>
            </div>
          )}
        </div>
      </div>

      {/* Account Selection */}
      <div className="card">
        <div className="card-header">
          <h2 className="text-lg font-semibold">Select Account</h2>
        </div>
        <div className="card-body">
          {accounts.length === 0 ? (
            <div className="text-center py-4">
              <p className="text-hone-500 mb-4">No accounts yet. Create one to get started.</p>
              <button onClick={() => setShowNewAccountModal(true)} className="btn-primary">
                <Plus className="w-4 h-4 mr-2" />
                Create Account
              </button>
            </div>
          ) : (
            <div className="space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                {accounts.map((account) => (
                  <div
                    key={account.id}
                    className={`p-4 rounded-lg border-2 transition-colors ${
                      selectedAccountId === account.id
                        ? "border-hone-500 bg-hone-50 dark:bg-hone-800"
                        : "border-hone-200 dark:border-hone-700 hover:border-hone-300 dark:hover:border-hone-600"
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <button
                        onClick={() => setSelectedAccountId(account.id)}
                        className="flex-1 text-left"
                      >
                        <div className="font-medium">{account.name}</div>
                        <div className="text-sm text-hone-500">{account.bank.toUpperCase()}</div>
                      </button>
                      <div className="flex items-center gap-1">
                        {selectedAccountId === account.id && (
                          <Check className="w-5 h-5 text-hone-500" />
                        )}
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setEditingAccount(account);
                          }}
                          className="p-1 text-hone-400 hover:text-hone-600 dark:hover:text-hone-200"
                          title="Edit account"
                        >
                          <Pencil className="w-4 h-4" />
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setDeletingAccountId(account.id);
                            setDeleteError(null);
                          }}
                          className="p-1 text-hone-400 hover:text-waste"
                          title="Delete account"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                    {/* Owner assignment */}
                    <div className="mt-2 pt-2 border-t border-hone-200 dark:border-hone-700">
                      {editingAccountEntity === account.id ? (
                        <select
                          value={account.entity_id ?? ""}
                          onChange={(e) => {
                            const val = e.target.value;
                            handleUpdateAccountEntity(account.id, val ? Number(val) : null);
                          }}
                          onBlur={() => setEditingAccountEntity(null)}
                          autoFocus
                          className="w-full text-sm rounded border border-hone-300 dark:border-hone-600 bg-white dark:bg-hone-800 px-2 py-1"
                        >
                          <option value="">No owner</option>
                          {entities.map((entity) => (
                            <option key={entity.id} value={entity.id}>
                              {entity.name}
                            </option>
                          ))}
                        </select>
                      ) : (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setEditingAccountEntity(account.id);
                          }}
                          className="flex items-center gap-1 text-xs text-hone-500 hover:text-hone-700 dark:hover:text-hone-300"
                        >
                          <User className="w-3 h-3" />
                          {getEntityName(account.entity_id) || "Assign owner..."}
                        </button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
              <button
                onClick={() => setShowNewAccountModal(true)}
                className="btn-ghost w-full"
              >
                <Plus className="w-4 h-4 mr-2" />
                Add New Account
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Import Button and Model Selector */}
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          {importResult && (
            <div
              className={`flex items-center gap-2 ${
                importResult.success ? "text-savings" : "text-waste"
              }`}
            >
              {importResult.success ? (
                <Check className="w-5 h-5" />
              ) : (
                <AlertTriangle className="w-5 h-5" />
              )}
              <span>{importResult.message}</span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-3">
          {/* Model selector */}
          {availableModels.length > 0 && (
            <div className="relative">
              <button
                onClick={() => setShowModelSelector(!showModelSelector)}
                className="flex items-center gap-2 px-3 py-2 text-sm
                          bg-hone-100 dark:bg-hone-800 rounded-lg
                          hover:bg-hone-200 dark:hover:bg-hone-700 transition-colors
                          text-hone-700 dark:text-hone-300"
                disabled={importing}
              >
                <Settings className="w-4 h-4" />
                <span className="max-w-[120px] truncate">
                  {loadingModels ? "Loading..." : selectedModel || "Model"}
                </span>
                <ChevronDown className="w-3 h-3" />
              </button>

              {showModelSelector && (
                <div className="absolute right-0 mt-1 w-56 bg-white dark:bg-hone-800
                              border border-hone-200 dark:border-hone-700 rounded-lg shadow-lg z-10
                              max-h-60 overflow-y-auto">
                  {availableModels.map((model) => (
                    <button
                      key={model}
                      onClick={() => {
                        setSelectedModel(model);
                        setShowModelSelector(false);
                      }}
                      className={`w-full text-left px-3 py-2 text-sm
                                  hover:bg-hone-100 dark:hover:bg-hone-700
                                  ${model === selectedModel
                                    ? "bg-hone-50 dark:bg-hone-700 text-hone-900 dark:text-hone-100"
                                    : "text-hone-700 dark:text-hone-300"
                                  }`}
                    >
                      <span className="truncate block">{model}</span>
                      {model === defaultModel && (
                        <span className="text-xs text-hone-400 ml-1">(default)</span>
                      )}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          <button
            onClick={handleImport}
            disabled={!file || !selectedAccountId || importing}
            className="btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {importing ? (
              <>
                <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                {importPhase}
              </>
            ) : (
              <>
                <Upload className="w-4 h-4 mr-2" />
                Import Transactions
              </>
            )}
          </button>
        </div>
      </div>

      {/* New Account Modal */}
      {showNewAccountModal && (
        <NewAccountModal
          onClose={() => setShowNewAccountModal(false)}
          onCreated={(account) => {
            onAccountCreated(account);
            setSelectedAccountId(account.id);
            setShowNewAccountModal(false);
          }}
        />
      )}

      {/* Edit Account Modal */}
      {editingAccount && (
        <EditAccountModal
          account={editingAccount}
          onClose={() => setEditingAccount(null)}
          onUpdated={() => {
            setEditingAccount(null);
            onAccountUpdated();
          }}
        />
      )}

      {/* Delete Confirmation Modal */}
      {deletingAccountId && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="card w-full max-w-md mx-4 animate-slide-up">
            <div className="card-header">
              <h2 className="text-lg font-semibold">Delete Account</h2>
            </div>
            <div className="card-body">
              <p className="text-hone-600 dark:text-hone-400">
                Are you sure you want to delete this account? This will also delete all transactions associated with this account. This action cannot be undone.
              </p>
              {deleteError && (
                <p className="mt-2 text-sm text-waste">{deleteError}</p>
              )}
            </div>
            <div className="card-body border-t border-hone-100 flex justify-end gap-2">
              <button
                onClick={() => {
                  setDeletingAccountId(null);
                  setDeleteError(null);
                }}
                className="btn-secondary"
              >
                Cancel
              </button>
              <button
                onClick={() => handleDeleteAccount(deletingAccountId)}
                className="btn-primary bg-waste hover:bg-waste/80"
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
