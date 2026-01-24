//! Entity command implementations (people, pets, vehicles, properties)

use anyhow::Result;
use hone_core::db::Database;
use hone_core::models::{EntityType, NewEntity};

use super::truncate;

/// List all entities
pub fn cmd_entities_list(db: &Database, show_archived: bool) -> Result<()> {
    let entities = db.list_entities(show_archived)?;

    if entities.is_empty() {
        println!("No entities found. Add one with:");
        println!("  hone entities add <name> --type person|pet|vehicle|property");
        return Ok(());
    }

    println!();
    println!("👤 Entities");
    println!("   ─────────────────────────────────────────────────────────────");
    println!(
        "   {:>4} │ {:20} │ {:10} │ {:6} │ {}",
        "ID", "Name", "Type", "Icon", "Status"
    );
    println!("   ─────┼──────────────────────┼────────────┼────────┼─────────────");

    for entity in entities {
        let status = if entity.archived {
            "archived"
        } else {
            "active"
        };
        let icon = entity.icon.as_deref().unwrap_or("-");
        println!(
            "   {:>4} │ {:20} │ {:10} │ {:6} │ {}",
            entity.id,
            truncate(&entity.name, 20),
            entity.entity_type.as_str(),
            icon,
            status
        );
    }

    Ok(())
}

/// List entities by type
pub fn cmd_entities_list_type(db: &Database, entity_type: &str) -> Result<()> {
    let etype: EntityType = entity_type.parse().map_err(|e: String| {
        anyhow::anyhow!("{} (valid types: person, pet, vehicle, property)", e)
    })?;

    let entities = db.list_entities_by_type(etype)?;

    if entities.is_empty() {
        println!("No {} entities found.", entity_type);
        return Ok(());
    }

    println!();
    println!("👤 {} Entities", entity_type);
    println!("   ─────────────────────────────────────────────────────────────");

    for entity in entities {
        let icon = entity.icon.as_deref().unwrap_or("");
        println!("   {} {} (id: {})", icon, entity.name, entity.id);
    }

    Ok(())
}

/// Add a new entity
pub fn cmd_entities_add(
    db: &Database,
    name: &str,
    entity_type: &str,
    icon: Option<&str>,
    color: Option<&str>,
) -> Result<()> {
    let etype: EntityType = entity_type.parse().map_err(|e: String| {
        anyhow::anyhow!("{} (valid types: person, pet, vehicle, property)", e)
    })?;

    let new_entity = NewEntity {
        name: name.to_string(),
        entity_type: etype,
        icon: icon.map(String::from),
        color: color.map(String::from),
    };

    let entity_id = db.create_entity(&new_entity)?;
    let icon_display = icon.map(|i| format!(" {}", i)).unwrap_or_default();
    println!(
        "✅ Created {} '{}'{} (id: {})",
        entity_type, name, icon_display, entity_id
    );

    Ok(())
}

/// Update an entity
pub fn cmd_entities_update(
    db: &Database,
    id: i64,
    name: Option<&str>,
    icon: Option<&str>,
    color: Option<&str>,
) -> Result<()> {
    let entity = db
        .get_entity(id)?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", id))?;

    db.update_entity(id, name, icon, color)?;

    let updated_name = name.unwrap_or(&entity.name);
    println!("✅ Updated entity '{}' (id: {})", updated_name, id);

    Ok(())
}

/// Archive an entity
pub fn cmd_entities_archive(db: &Database, id: i64) -> Result<()> {
    let entity = db
        .get_entity(id)?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", id))?;

    db.archive_entity(id)?;
    println!("✅ Archived entity '{}' (id: {})", entity.name, id);

    Ok(())
}

/// Unarchive an entity
pub fn cmd_entities_unarchive(db: &Database, id: i64) -> Result<()> {
    let entity = db
        .get_entity(id)?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", id))?;

    db.unarchive_entity(id)?;
    println!("✅ Unarchived entity '{}' (id: {})", entity.name, id);

    Ok(())
}

/// Delete an entity
pub fn cmd_entities_delete(db: &Database, id: i64, force: bool) -> Result<()> {
    let entity = db
        .get_entity(id)?
        .ok_or_else(|| anyhow::anyhow!("Entity not found: {}", id))?;

    // Check if entity has splits
    let split_count = db.count_splits_by_entity(id)?;

    if split_count > 0 && !force {
        anyhow::bail!(
            "Entity '{}' has {} associated splits. Use --force to delete anyway.",
            entity.name,
            split_count
        );
    }

    db.delete_entity(id)?;
    println!("✅ Deleted entity '{}' (id: {})", entity.name, id);

    Ok(())
}

/// Resolve an entity by name or ID
#[allow(dead_code)] // Used in tests, kept for future CLI commands
pub fn resolve_entity_arg(db: &Database, name_or_id: &str) -> Result<hone_core::models::Entity> {
    // First try as an ID
    if let Ok(id) = name_or_id.parse::<i64>() {
        if let Some(entity) = db.get_entity(id)? {
            return Ok(entity);
        }
    }

    // Try to find by name (include archived to resolve by name)
    let entities = db.list_entities(true)?;
    for entity in entities {
        if entity.name.eq_ignore_ascii_case(name_or_id) {
            return Ok(entity);
        }
    }

    anyhow::bail!("Entity not found: {}", name_or_id)
}
