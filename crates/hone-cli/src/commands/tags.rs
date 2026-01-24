//! Tag and rule command implementations

use anyhow::Result;
use hone_core::db::Database;
use hone_core::models::{PatternType, TagSource};

use super::truncate;

pub fn cmd_tags_list(db: &Database) -> Result<()> {
    let tree = db.get_tag_tree()?;

    if tree.is_empty() {
        println!("No tags found. Run 'hone init' to seed default tags.");
        return Ok(());
    }

    println!();
    println!("🏷️  Tags");
    println!("   ─────────────────────────────────────────────────────────────");

    fn print_tag(tag: &hone_core::models::TagWithPath, indent: usize) {
        let prefix = "  ".repeat(indent);
        let patterns = tag
            .tag
            .auto_patterns
            .as_ref()
            .map(|p| format!(" ({})", p))
            .unwrap_or_default();
        let color = tag
            .tag
            .color
            .as_ref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();
        println!("   {}• {}{}{}", prefix, tag.tag.name, color, patterns);

        for child in &tag.children {
            print_tag(child, indent + 1);
        }
    }

    for root in &tree {
        print_tag(root, 0);
    }

    Ok(())
}

pub fn cmd_tags_add(
    db: &Database,
    path: &str,
    color: Option<&str>,
    patterns: Option<&str>,
) -> Result<()> {
    // Parse path to get parent and name
    let (parent_path, name) = if let Some(dot_pos) = path.rfind('.') {
        (Some(&path[..dot_pos]), &path[dot_pos + 1..])
    } else {
        (None, path)
    };

    // Resolve parent if specified
    let parent_id = if let Some(parent_path) = parent_path {
        let parent = db
            .get_tag_by_path(parent_path)?
            .ok_or_else(|| anyhow::anyhow!("Parent tag not found: {}", parent_path))?;
        Some(parent.id)
    } else {
        None
    };

    let tag_id = db.create_tag(name, parent_id, color, None, patterns)?;
    println!("✅ Created tag '{}' (id: {})", path, tag_id);

    Ok(())
}

pub fn cmd_tags_rename(db: &Database, old_name: &str, new_name: &str) -> Result<()> {
    let tag = resolve_tag_arg(db, old_name)?;

    db.update_tag(tag.id, Some(new_name), None, None, None, None)?;
    println!("✅ Renamed '{}' to '{}'", old_name, new_name);

    Ok(())
}

pub fn cmd_tags_move(db: &Database, tag_name: &str, to: &str) -> Result<()> {
    let tag = resolve_tag_arg(db, tag_name)?;

    let new_parent_id = if to.eq_ignore_ascii_case("root") {
        None
    } else {
        let parent = resolve_tag_arg(db, to)?;
        Some(parent.id)
    };

    db.update_tag(tag.id, None, Some(new_parent_id), None, None, None)?;

    let new_parent_name = if new_parent_id.is_some() { to } else { "root" };
    println!("✅ Moved '{}' under '{}'", tag_name, new_parent_name);

    Ok(())
}

pub fn cmd_tags_delete(db: &Database, tag_name: &str, force: bool, to_parent: bool) -> Result<()> {
    let tag = resolve_tag_arg(db, tag_name)?;

    // Check if tag has transactions
    let tx_count = db.count_transactions_by_tag(tag.id)?;
    if tx_count > 0 && !force {
        anyhow::bail!(
            "Tag '{}' has {} transactions. Use --force to delete anyway, or --to-parent to move them.",
            tag_name,
            tx_count
        );
    }

    let result = db.delete_tag(tag.id, to_parent)?;
    println!(
        "✅ Deleted tag '{}' ({} transactions moved, {} children affected)",
        tag_name, result.transactions_moved, result.children_affected
    );

    Ok(())
}

pub fn cmd_tags_merge(db: &Database, source: &str, target: &str) -> Result<()> {
    let source_tag = resolve_tag_arg(db, source)?;
    let target_tag = resolve_tag_arg(db, target)?;

    let moved = db.merge_tags(source_tag.id, target_tag.id)?;
    println!(
        "✅ Merged '{}' into '{}' ({} transactions moved)",
        source, target, moved
    );

    Ok(())
}

/// Resolve a tag argument (name or path) to a Tag
pub fn resolve_tag_arg(db: &Database, name_or_path: &str) -> Result<hone_core::models::Tag> {
    // First try as a path
    if name_or_path.contains('.') {
        if let Some(tag) = db.get_tag_by_path(name_or_path)? {
            return Ok(tag);
        }
    }

    // Try to resolve by name
    if let Some(tag) = db.resolve_tag(name_or_path)? {
        return Ok(tag);
    }

    anyhow::bail!("Tag not found: {}", name_or_path)
}

// ========== Rules Commands ==========

pub fn cmd_rules_list(db: &Database) -> Result<()> {
    let rules = db.list_tag_rules()?;

    if rules.is_empty() {
        println!("No rules defined. Add one with:");
        println!("  hone rules add <tag> <pattern> [--type contains|regex|exact]");
        return Ok(());
    }

    println!();
    println!("📋 Tag Rules");
    println!("   ─────────────────────────────────────────────────────────────");
    println!(
        "   {:>4} │ {:>4} │ {:20} │ {:10} │ {}",
        "ID", "Pri", "Tag", "Type", "Pattern"
    );
    println!("   ─────┼──────┼──────────────────────┼────────────┼─────────────────");

    for rule in rules {
        println!(
            "   {:>4} │ {:>4} │ {:20} │ {:10} │ {}",
            rule.rule.id,
            rule.rule.priority,
            truncate(&rule.tag_path, 20),
            rule.rule.pattern_type.as_str(),
            truncate(&rule.rule.pattern, 30)
        );
    }

    Ok(())
}

pub fn cmd_rules_add(
    db: &Database,
    tag_name: &str,
    pattern: &str,
    pattern_type_str: &str,
    priority: i32,
) -> Result<()> {
    let tag = resolve_tag_arg(db, tag_name)?;

    let pattern_type: PatternType = pattern_type_str
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{} (valid types: contains, regex, exact)", e))?;

    let rule_id = db.create_tag_rule(tag.id, pattern, pattern_type, priority)?;
    println!(
        "✅ Created rule #{} for tag '{}': {} ({})",
        rule_id,
        tag_name,
        pattern,
        pattern_type.as_str()
    );

    Ok(())
}

pub fn cmd_rules_delete(db: &Database, id: i64) -> Result<()> {
    db.delete_tag_rule(id)?;
    println!("✅ Deleted rule #{}", id);

    Ok(())
}

pub fn cmd_rules_test(db: &Database, description: &str) -> Result<()> {
    use hone_core::tags::test_rules_against;

    let matches = test_rules_against(db, description)?;

    if matches.is_empty() {
        println!("No rules match \"{}\"", description);
        return Ok(());
    }

    println!();
    println!("🔍 Rules matching \"{}\":", description);
    println!("   ─────────────────────────────────────────────────────────────");

    for (rule, tag) in matches {
        println!(
            "   Rule #{} (priority {}) -> {} ({}: {})",
            rule.id,
            rule.priority,
            tag.name,
            rule.pattern_type.as_str(),
            rule.pattern
        );
    }

    Ok(())
}

// ========== Transaction Tagging Commands ==========

pub fn cmd_tag(db: &Database, transaction_id: i64, tag_name: &str) -> Result<()> {
    let tag = resolve_tag_arg(db, tag_name)?;

    db.add_transaction_tag(transaction_id, tag.id, TagSource::Manual, None)?;
    println!(
        "✅ Tagged transaction #{} with '{}'",
        transaction_id, tag_name
    );

    Ok(())
}

pub fn cmd_untag(db: &Database, transaction_id: i64, tag_name: &str) -> Result<()> {
    let tag = resolve_tag_arg(db, tag_name)?;

    db.remove_transaction_tag(transaction_id, tag.id)?;
    println!(
        "✅ Removed tag '{}' from transaction #{}",
        tag_name, transaction_id
    );

    Ok(())
}
