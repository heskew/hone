import { useState, useEffect } from "react";
import { Plus, RefreshCw, AlertTriangle, X } from "lucide-react";
import { api } from "../../api";
import type { TagWithPath, TagRule } from "../../types";
import { TagTreeNode } from "./TagTreeNode";
import {
  CreateTagModal,
  EditTagModal,
  DeleteTagModal,
  MoveTagModal,
  MergeTagModal,
  CreateRuleModal,
} from "./modals";

export function TagsPage() {
  const [tagsTree, setTagsTree] = useState<TagWithPath[]>([]);
  const [rules, setRules] = useState<TagRule[]>([]);
  const [selectedTag, setSelectedTag] = useState<TagWithPath | null>(null);
  const [expandedTags, setExpandedTags] = useState<Set<number>>(new Set());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Modal states
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showEditModal, setShowEditModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [showMoveModal, setShowMoveModal] = useState(false);
  const [showMergeModal, setShowMergeModal] = useState(false);
  const [showCreateRuleModal, setShowCreateRuleModal] = useState(false);
  const [createParentId, setCreateParentId] = useState<number | null>(null);

  const loadTags = async () => {
    try {
      setLoading(true);
      setError(null);
      const [tree, rulesData] = await Promise.all([
        api.getTagsTree(),
        api.getTagRules(),
      ]);
      setTagsTree(tree);
      setRules(rulesData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load tags");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadTags();
  }, []);

  const toggleExpand = (tagId: number) => {
    setExpandedTags((prev) => {
      const next = new Set(prev);
      if (next.has(tagId)) {
        next.delete(tagId);
      } else {
        next.add(tagId);
      }
      return next;
    });
  };

  const handleCreateTag = () => {
    setCreateParentId(null);
    setShowCreateModal(true);
  };

  const handleCreateChildTag = (parentId: number) => {
    setCreateParentId(parentId);
    setShowCreateModal(true);
  };

  const handleTagCreated = async () => {
    setShowCreateModal(false);
    setCreateParentId(null);
    await loadTags();
  };

  const handleTagRenamed = async () => {
    setShowEditModal(false);
    await loadTags();
    // Update selected tag if it was renamed
    if (selectedTag) {
      const tree = await api.getTagsTree();
      const findTag = (tags: TagWithPath[]): TagWithPath | null => {
        for (const t of tags) {
          if (t.id === selectedTag.id) return t;
          const found = findTag(t.children ?? []);
          if (found) return found;
        }
        return null;
      };
      setSelectedTag(findTag(tree));
    }
  };

  const handleTagDeleted = async () => {
    setShowDeleteModal(false);
    setSelectedTag(null);
    await loadTags();
  };

  const handleTagMoved = async () => {
    setShowMoveModal(false);
    await loadTags();
    // Update selected tag after move
    if (selectedTag) {
      const tree = await api.getTagsTree();
      const findTag = (tags: TagWithPath[]): TagWithPath | null => {
        for (const t of tags) {
          if (t.id === selectedTag.id) return t;
          const found = findTag(t.children ?? []);
          if (found) return found;
        }
        return null;
      };
      setSelectedTag(findTag(tree));
    }
  };

  const handleTagMerged = async () => {
    setShowMergeModal(false);
    setSelectedTag(null);
    await loadTags();
  };

  const handleRuleCreated = async () => {
    setShowCreateRuleModal(false);
    const rulesData = await api.getTagRules();
    setRules(rulesData);
  };

  const handleDeleteRule = async (ruleId: number) => {
    try {
      await api.deleteTagRule(ruleId);
      setRules(rules.filter((r) => r.id !== ruleId));
    } catch (err) {
      console.error("Failed to delete rule:", err);
    }
  };

  // Flatten tags for dropdowns
  const flattenTags = (tags: TagWithPath[]): TagWithPath[] => {
    const result: TagWithPath[] = [];
    const traverse = (t: TagWithPath) => {
      result.push(t);
      (t.children ?? []).forEach(traverse);
    };
    tags.forEach(traverse);
    return result;
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <RefreshCw className="w-5 h-5 animate-spin text-hone-500" />
        <span className="ml-2 text-hone-500">Loading tags...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="card p-8 text-center">
        <AlertTriangle className="w-12 h-12 text-attention mx-auto mb-4" />
        <h2 className="text-lg font-semibold mb-2">Failed to load tags</h2>
        <p className="text-hone-500 mb-4">{error}</p>
        <button onClick={loadTags} className="btn-primary">
          <RefreshCw className="w-4 h-4 mr-2" />
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-hone-900 dark:text-hone-100">Tag Management</h1>
        <button onClick={handleCreateTag} className="btn-primary">
          <Plus className="w-4 h-4 mr-2" />
          New Root Tag
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Tag Tree */}
        <div className="lg:col-span-2 card">
          <div className="card-header">
            <h2 className="font-semibold">Tag Hierarchy</h2>
          </div>
          <div className="card-body">
            {tagsTree.length === 0 ? (
              <p className="text-hone-400 text-center py-8">No tags yet. Create one to get started.</p>
            ) : (
              <div className="space-y-1">
                {tagsTree.map((tag) => (
                  <TagTreeNode
                    key={tag.id}
                    tag={tag}
                    selectedId={selectedTag?.id}
                    expandedTags={expandedTags}
                    onSelect={setSelectedTag}
                    onToggleExpand={toggleExpand}
                    onCreateChild={handleCreateChildTag}
                  />
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Tag Details */}
        <div className="card">
          <div className="card-header">
            <h2 className="font-semibold">Tag Details</h2>
          </div>
          <div className="card-body">
            {selectedTag ? (
              <div className="space-y-4">
                <div>
                  <div className="flex items-center gap-2 mb-1">
                    {selectedTag.color && (
                      <span
                        className="w-4 h-4 rounded-full"
                        style={{ backgroundColor: selectedTag.color }}
                      />
                    )}
                    <span className="font-semibold text-lg">{selectedTag.name}</span>
                  </div>
                  <p className="text-sm text-hone-400">{selectedTag.path}</p>
                </div>

                <div className="text-sm space-y-2">
                  <div className="flex justify-between">
                    <span className="text-hone-500">Depth</span>
                    <span>{selectedTag.depth}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-hone-500">Children</span>
                    <span>{(selectedTag.children ?? []).length}</span>
                  </div>
                  {selectedTag.auto_patterns && (
                    <div>
                      <span className="text-hone-500 block mb-1">Quick Patterns</span>
                      <code className="text-xs bg-hone-50 dark:bg-hone-800 px-2 py-1 rounded block break-all">
                        {selectedTag.auto_patterns}
                      </code>
                      <p className="text-xs text-hone-400 mt-1">
                        Pipe-separated, checked after Rules
                      </p>
                    </div>
                  )}
                </div>

                <div className="border-t border-hone-100 pt-4 space-y-2">
                  <button
                    onClick={() => setShowEditModal(true)}
                    className="btn-secondary w-full justify-center"
                  >
                    Edit
                  </button>
                  <button
                    onClick={() => setShowMoveModal(true)}
                    className="btn-secondary w-full justify-center"
                  >
                    Move
                  </button>
                  <button
                    onClick={() => setShowMergeModal(true)}
                    className="btn-secondary w-full justify-center"
                  >
                    Merge Into...
                  </button>
                  <button
                    onClick={() => setShowDeleteModal(true)}
                    className="btn-secondary w-full justify-center text-waste hover:bg-red-50"
                  >
                    Delete
                  </button>
                </div>
              </div>
            ) : (
              <p className="text-hone-400 text-center py-8">
                Select a tag to view details
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Rules Section */}
      <div className="card">
        <div className="card-header flex items-center justify-between">
          <div>
            <h2 className="font-semibold">Auto-Tagging Rules</h2>
            <p className="text-xs text-hone-400 mt-0.5">
              Rules are checked first (by priority), then tag patterns, then Ollama AI (if configured)
            </p>
          </div>
          <button onClick={() => setShowCreateRuleModal(true)} className="btn-ghost text-sm">
            <Plus className="w-4 h-4 mr-1" />
            Add Rule
          </button>
        </div>
        <div className="card-body">
          {rules.length === 0 ? (
            <p className="text-hone-400 text-center py-4">No rules defined yet. Rules give you precise control over auto-tagging.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-hone-100">
                    <th className="text-left py-2 px-3 text-hone-500 font-medium">Tag</th>
                    <th className="text-left py-2 px-3 text-hone-500 font-medium">Pattern</th>
                    <th className="text-left py-2 px-3 text-hone-500 font-medium">Type</th>
                    <th className="text-left py-2 px-3 text-hone-500 font-medium">Priority</th>
                    <th className="text-right py-2 px-3"></th>
                  </tr>
                </thead>
                <tbody>
                  {rules.map((rule) => (
                    <tr key={rule.id} className="border-b border-hone-50 dark:border-hone-700 hover:bg-hone-100 dark:hover:bg-hone-700">
                      <td className="py-2 px-3">{rule.tag_path}</td>
                      <td className="py-2 px-3">
                        <code className="text-xs bg-hone-100 px-1.5 py-0.5 rounded">
                          {rule.pattern}
                        </code>
                      </td>
                      <td className="py-2 px-3 capitalize">{rule.pattern_type}</td>
                      <td className="py-2 px-3">{rule.priority}</td>
                      <td className="py-2 px-3 text-right">
                        <button
                          onClick={() => handleDeleteRule(rule.id)}
                          className="text-hone-400 hover:text-waste"
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Modals */}
      {showCreateModal && (
        <CreateTagModal
          parentId={createParentId}
          tags={flattenTags(tagsTree)}
          onClose={() => {
            setShowCreateModal(false);
            setCreateParentId(null);
          }}
          onCreated={handleTagCreated}
        />
      )}

      {showEditModal && selectedTag && (
        <EditTagModal
          tag={selectedTag}
          onClose={() => setShowEditModal(false)}
          onSaved={handleTagRenamed}
        />
      )}

      {showDeleteModal && selectedTag && (
        <DeleteTagModal
          tag={selectedTag}
          onClose={() => setShowDeleteModal(false)}
          onDeleted={handleTagDeleted}
        />
      )}

      {showMoveModal && selectedTag && (
        <MoveTagModal
          tag={selectedTag}
          tags={flattenTags(tagsTree)}
          onClose={() => setShowMoveModal(false)}
          onMoved={handleTagMoved}
        />
      )}

      {showMergeModal && selectedTag && (
        <MergeTagModal
          tag={selectedTag}
          tags={flattenTags(tagsTree)}
          onClose={() => setShowMergeModal(false)}
          onMerged={handleTagMerged}
        />
      )}

      {showCreateRuleModal && (
        <CreateRuleModal
          tags={flattenTags(tagsTree)}
          onClose={() => setShowCreateRuleModal(false)}
          onCreated={handleRuleCreated}
        />
      )}
    </div>
  );
}
