use anyhow::{Context, Result};
use git2::Repository;

pub const ARC_REFS_PREFIX: &str = "refs/arc";

/// Read a JSON blob from a ref under refs/arc/.
pub fn read_ref(repo: &Repository, ref_path: &str) -> Result<Option<String>> {
    let full_ref = format!("{ARC_REFS_PREFIX}/{ref_path}");
    let reference = match repo.find_reference(&full_ref) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let blob = reference
        .peel_to_blob()
        .context("ref does not point to a blob")?;
    let content = std::str::from_utf8(blob.content())
        .context("ref blob is not valid UTF-8")?;
    Ok(Some(content.to_string()))
}

/// Write a JSON blob to a ref under refs/arc/.
pub fn write_ref(repo: &Repository, ref_path: &str, content: &str) -> Result<()> {
    let full_ref = format!("{ARC_REFS_PREFIX}/{ref_path}");
    let oid = repo.blob(content.as_bytes())?;
    repo.reference(&full_ref, oid, true, "arc: update ref")?;
    Ok(())
}

/// Delete a ref under refs/arc/.
pub fn delete_ref(repo: &Repository, ref_path: &str) -> Result<()> {
    let full_ref = format!("{ARC_REFS_PREFIX}/{ref_path}");
    if let Ok(mut reference) = repo.find_reference(&full_ref) {
        reference.delete()?;
    }
    Ok(())
}

/// List all ref names matching a prefix under refs/arc/.
pub fn list_refs(repo: &Repository, prefix: &str) -> Result<Vec<String>> {
    let full_prefix = format!("{ARC_REFS_PREFIX}/{prefix}");
    let mut names = Vec::new();
    for reference in repo.references_glob(&format!("{full_prefix}*"))? {
        let reference = reference?;
        if let Some(name) = reference.name() {
            names.push(name.to_string());
        }
    }
    Ok(names)
}
