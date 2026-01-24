import { X } from "lucide-react";
import { useState } from "react";
import { api } from "../../api";
import type { PatternType, TagWithPath } from "../../types";

// Create Tag Modal
interface CreateTagModalProps {
  parentId: number | null;
  tags: TagWithPath[];
  onClose: () => void;
  onCreated: () => void;
}

export function CreateTagModal({ parentId, tags, onClose, onCreated }: CreateTagModalProps) {
  const [name, setName] = useState("");
  const [selectedParentId, setSelectedParentId] = useState<number | null>(parentId);
  const [color, setColor] = useState("");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    try {
      setCreating(true);
      setError(null);
      await api.createTag({
        name: name.trim(),
        parent_id: selectedParentId ?? undefined,
        color: color || undefined,
      });
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create tag");
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold">Create Tag</h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="card-body space-y-4">
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Tag Name
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g., Groceries, Entertainment"
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Parent Tag
              </label>
              <select
                value={selectedParentId ?? ""}
                onChange={(e) =>
                  setSelectedParentId(e.target.value ? Number(e.target.value) : null)
                }
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
              >
                <option value="">Root (no parent)</option>
                {tags.map((t) => (
                  <option key={t.id} value={t.id}>
                    {"  ".repeat(t.depth)}{t.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Color (optional)
              </label>
              <input
                type="color"
                value={color || "#6b7280"}
                onChange={(e) => setColor(e.target.value)}
                className="w-full h-10 border border-hone-200 rounded-lg cursor-pointer"
              />
            </div>
            {error && <div className="text-sm text-waste">{error}</div>}
          </div>
          <div className="card-body border-t border-hone-100 flex justify-end gap-2">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button
              type="submit"
              disabled={!name.trim() || creating}
              className="btn-primary disabled:opacity-50"
            >
              {creating ? "Creating..." : "Create Tag"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// Edit Tag Modal
interface EditTagModalProps {
  tag: TagWithPath;
  onClose: () => void;
  onSaved: () => void;
}

export function EditTagModal({ tag, onClose, onSaved }: EditTagModalProps) {
  const [name, setName] = useState(tag.name);
  const [autoPatterns, setAutoPatterns] = useState(tag.auto_patterns ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const hasChanges = name.trim() !== tag.name || autoPatterns !== (tag.auto_patterns ?? "");

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !hasChanges) return;

    try {
      setSaving(true);
      setError(null);
      await api.updateTag(tag.id, {
        name: name.trim(),
        auto_patterns: autoPatterns.trim() || null,
      });
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update tag");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold">Edit Tag</h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="card-body space-y-4">
            <p className="text-sm text-hone-500">
              Path: <span className="font-medium">{tag.path}</span>
            </p>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Name
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Quick Patterns
              </label>
              <input
                type="text"
                value={autoPatterns}
                onChange={(e) => setAutoPatterns(e.target.value)}
                placeholder="e.g., NETFLIX|HULU|DISNEY"
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
              />
              <p className="mt-1 text-xs text-hone-400">
                Pipe-separated list of text to match (case-insensitive). For more control, use Rules below.
              </p>
            </div>
            {error && <div className="text-sm text-waste">{error}</div>}
          </div>
          <div className="card-body border-t border-hone-100 flex justify-end gap-2">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button
              type="submit"
              disabled={!name.trim() || !hasChanges || saving}
              className="btn-primary disabled:opacity-50"
            >
              {saving ? "Saving..." : "Save"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// Delete Tag Modal
interface DeleteTagModalProps {
  tag: TagWithPath;
  onClose: () => void;
  onDeleted: () => void;
}

export function DeleteTagModal({ tag, onClose, onDeleted }: DeleteTagModalProps) {
  const [reparent, setReparent] = useState(true);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleDelete = async () => {
    try {
      setDeleting(true);
      setError(null);
      await api.deleteTag(tag.id, reparent);
      onDeleted();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete tag");
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold text-waste">Delete Tag</h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="card-body space-y-4">
          <p>
            Are you sure you want to delete <strong>{tag.name}</strong>?
          </p>
          {(tag.children ?? []).length > 0 && (
            <p className="text-sm text-hone-500">
              This tag has {(tag.children ?? []).length} child tag(s).
            </p>
          )}
          <div className="space-y-2">
            <label className="flex items-center gap-2">
              <input
                type="radio"
                name="reparent"
                checked={reparent}
                onChange={() => setReparent(true)}
              />
              <span className="text-sm">
                Move children and transactions to parent tag
              </span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="radio"
                name="reparent"
                checked={!reparent}
                onChange={() => setReparent(false)}
              />
              <span className="text-sm">
                Delete children and remove tags from transactions
              </span>
            </label>
          </div>
          {error && <div className="text-sm text-waste">{error}</div>}
        </div>
        <div className="card-body border-t border-hone-100 flex justify-end gap-2">
          <button onClick={onClose} className="btn-secondary">
            Cancel
          </button>
          <button
            onClick={handleDelete}
            disabled={deleting}
            className="px-4 py-2 bg-waste text-white rounded-lg hover:bg-red-600 disabled:opacity-50"
          >
            {deleting ? "Deleting..." : "Delete Tag"}
          </button>
        </div>
      </div>
    </div>
  );
}

// Move Tag Modal
interface MoveTagModalProps {
  tag: TagWithPath;
  tags: TagWithPath[];
  onClose: () => void;
  onMoved: () => void;
}

export function MoveTagModal({ tag, tags, onClose, onMoved }: MoveTagModalProps) {
  const [newParentId, setNewParentId] = useState<number | null>(tag.parent_id);
  const [moving, setMoving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Filter out the tag itself and its descendants
  const isDescendant = (t: TagWithPath): boolean => {
    if (t.id === tag.id) return true;
    return t.path.startsWith(tag.path + " > ");
  };

  const availableParents = tags.filter((t) => !isDescendant(t));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newParentId === tag.parent_id) return;

    try {
      setMoving(true);
      setError(null);
      await api.updateTag(tag.id, { parent_id: newParentId });
      onMoved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to move tag");
    } finally {
      setMoving(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold">Move Tag</h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="card-body space-y-4">
            <p className="text-sm text-hone-500">
              Moving: <strong>{tag.path}</strong>
            </p>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                New Parent
              </label>
              <select
                value={newParentId ?? ""}
                onChange={(e) =>
                  setNewParentId(e.target.value ? Number(e.target.value) : null)
                }
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
              >
                <option value="">Root (no parent)</option>
                {availableParents.map((t) => (
                  <option key={t.id} value={t.id}>
                    {"  ".repeat(t.depth)}{t.name}
                  </option>
                ))}
              </select>
            </div>
            {error && <div className="text-sm text-waste">{error}</div>}
          </div>
          <div className="card-body border-t border-hone-100 flex justify-end gap-2">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button
              type="submit"
              disabled={newParentId === tag.parent_id || moving}
              className="btn-primary disabled:opacity-50"
            >
              {moving ? "Moving..." : "Move Tag"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// Merge Tag Modal
interface MergeTagModalProps {
  tag: TagWithPath;
  tags: TagWithPath[];
  onClose: () => void;
  onMerged: () => void;
}

export function MergeTagModal({ tag, tags, onClose, onMerged }: MergeTagModalProps) {
  const [targetId, setTargetId] = useState<number | null>(null);
  const [merging, setMerging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Filter out the tag itself
  const availableTargets = tags.filter((t) => t.id !== tag.id);

  const handleMerge = async () => {
    if (!targetId) return;

    try {
      setMerging(true);
      setError(null);
      // Merge by moving children and then deleting the source tag
      // First update all children to have the target as parent
      for (const child of tag.children ?? []) {
        await api.updateTag(child.id, { parent_id: targetId });
      }
      // Then delete the source tag (with reparent=true to move transactions)
      await api.deleteTag(tag.id, true);
      onMerged();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to merge tags");
    } finally {
      setMerging(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold">Merge Tag</h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="card-body space-y-4">
          <p className="text-sm text-hone-500">
            Merge <strong>{tag.name}</strong> into another tag. All transactions
            and children will be moved to the target tag.
          </p>
          <div>
            <label className="block text-sm font-medium text-hone-700 mb-1">
              Target Tag
            </label>
            <select
              value={targetId ?? ""}
              onChange={(e) =>
                setTargetId(e.target.value ? Number(e.target.value) : null)
              }
              className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
            >
              <option value="">Select a tag...</option>
              {availableTargets.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.path}
                </option>
              ))}
            </select>
          </div>
          {error && <div className="text-sm text-waste">{error}</div>}
        </div>
        <div className="card-body border-t border-hone-100 flex justify-end gap-2">
          <button onClick={onClose} className="btn-secondary">
            Cancel
          </button>
          <button
            onClick={handleMerge}
            disabled={!targetId || merging}
            className="btn-primary disabled:opacity-50"
          >
            {merging ? "Merging..." : "Merge"}
          </button>
        </div>
      </div>
    </div>
  );
}

// Create Rule Modal
interface CreateRuleModalProps {
  tags: TagWithPath[];
  onClose: () => void;
  onCreated: () => void;
}

export function CreateRuleModal({ tags, onClose, onCreated }: CreateRuleModalProps) {
  const [tagId, setTagId] = useState<number | null>(null);
  const [pattern, setPattern] = useState("");
  const [patternType, setPatternType] = useState<PatternType>("contains");
  const [priority, setPriority] = useState(0);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!tagId || !pattern.trim()) return;

    try {
      setCreating(true);
      setError(null);
      await api.createTagRule({
        tag_id: tagId,
        pattern: pattern.trim(),
        pattern_type: patternType,
        priority,
      });
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create rule");
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="card w-full max-w-md mx-4 animate-slide-up">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold">Create Auto-Tag Rule</h2>
          <button onClick={onClose} className="p-1 text-hone-400 hover:text-hone-600">
            <X className="w-5 h-5" />
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="card-body space-y-4">
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Tag
              </label>
              <select
                value={tagId ?? ""}
                onChange={(e) =>
                  setTagId(e.target.value ? Number(e.target.value) : null)
                }
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
              >
                <option value="">Select a tag...</option>
                {tags.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.path}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Pattern
              </label>
              <input
                type="text"
                value={pattern}
                onChange={(e) => setPattern(e.target.value)}
                placeholder={patternType === "regex" ? "e.g., AMAZON.*PRIME" : "e.g., STARBUCKS"}
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
              />
              <p className="mt-1 text-xs text-hone-400">
                {patternType === "contains" && "Use pipe (|) for multiple patterns: STARBUCKS|DUNKIN|PEETS"}
                {patternType === "exact" && "Must match the full transaction description exactly"}
                {patternType === "regex" && "Regular expression - use .* for wildcards, ^ for start, $ for end"}
              </p>
            </div>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Pattern Type
              </label>
              <select
                value={patternType}
                onChange={(e) => setPatternType(e.target.value as PatternType)}
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
              >
                <option value="contains">Contains (case-insensitive)</option>
                <option value="exact">Exact Match</option>
                <option value="regex">Regex</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-hone-700 mb-1">
                Priority
              </label>
              <input
                type="number"
                value={priority}
                onChange={(e) => setPriority(parseInt(e.target.value) || 0)}
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
              />
              <p className="mt-1 text-xs text-hone-400">
                Higher priority rules are checked first
              </p>
            </div>
            {error && <div className="text-sm text-waste">{error}</div>}
          </div>
          <div className="card-body border-t border-hone-100 flex justify-end gap-2">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button
              type="submit"
              disabled={!tagId || !pattern.trim() || creating}
              className="btn-primary disabled:opacity-50"
            >
              {creating ? "Creating..." : "Create Rule"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
