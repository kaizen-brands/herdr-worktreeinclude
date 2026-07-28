use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCreatedPaths {
    pub source_root: String,
    pub target_root: String,
}

/// Extract source (main) and target (new worktree) roots from HERDR_PLUGIN_EVENT_JSON.
/// Field names confirmed against herdr 0.7.x community payloads.
pub fn parse_worktree_created_paths(raw: Option<&str>) -> Option<WorktreeCreatedPaths> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }

    let event: Value = serde_json::from_str(raw).ok()?;
    if !event.is_object() {
        return None;
    }

    let data = event
        .get("data")
        .filter(|d| d.is_object())
        .unwrap_or(&event);

    let target_root = first_absolute_path(&[
        dig(data, &["worktree", "path"]),
        dig(&event, &["worktree", "path"]),
        dig(data, &["worktree", "checkout_path"]),
    ])?;

    let source_root = first_absolute_path(&[
        dig(data, &["workspace", "worktree", "repo_root"]),
        dig(data, &["workspace", "worktree", "checkout_path"]),
        dig(data, &["source_worktree", "path"]),
    ])
    .unwrap_or_default();

    Some(WorktreeCreatedPaths {
        source_root,
        target_root,
    })
}

fn dig(obj: &Value, path: &[&str]) -> Option<Value> {
    let mut cur = obj;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur.clone())
}

fn first_absolute_path(candidates: &[Option<Value>]) -> Option<String> {
    for c in candidates {
        if let Some(Value::String(s)) = c {
            if s.starts_with('/') {
                return Some(s.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_herdr_0_7_nested_payload() {
        let raw = r#"{
            "event": "worktree_created",
            "data": {
                "workspace": {
                    "workspace_id": "w1",
                    "worktree": {
                        "repo_root": "/repo/main",
                        "checkout_path": "/repo/main"
                    }
                },
                "worktree": {
                    "path": "/repo/worktrees/feature",
                    "branch": "feature"
                }
            }
        }"#;
        assert_eq!(
            parse_worktree_created_paths(Some(raw)),
            Some(WorktreeCreatedPaths {
                source_root: "/repo/main".into(),
                target_root: "/repo/worktrees/feature".into(),
            })
        );
    }

    #[test]
    fn returns_none_for_empty_or_invalid_json() {
        assert_eq!(parse_worktree_created_paths(None), None);
        assert_eq!(parse_worktree_created_paths(Some("")), None);
        assert_eq!(parse_worktree_created_paths(Some("{")), None);
    }

    #[test]
    fn accepts_target_only_when_source_missing() {
        let raw = r#"{"data":{"worktree":{"path":"/tmp/wt"}}}"#;
        assert_eq!(
            parse_worktree_created_paths(Some(raw)),
            Some(WorktreeCreatedPaths {
                source_root: String::new(),
                target_root: "/tmp/wt".into(),
            })
        );
    }
}
