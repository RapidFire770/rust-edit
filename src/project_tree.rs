// Expandable project-folder tree. Listing comes from the file browser.
// Paint and input live elsewhere. This module only reads directories.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ui::popups::file_browser::read_children;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatRow {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandOutcome {
    Expanded,
    Collapsed,
    OutsideRoot,
    Loop,
    NotADirectory,
    Io(String),
}

#[derive(Debug, Clone)]
pub struct ProjectTree {
    pub root: PathBuf,
    show_hidden: bool,
    nodes: Vec<TreeNode>,
}

impl ProjectTree {
    pub fn load(root: &Path, show_hidden: bool) -> Result<Self, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("Cannot open that folder".to_string());
        }
        let nodes = read_nodes(&root, show_hidden)?;
        Ok(Self {
            root,
            show_hidden,
            nodes,
        })
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn set_show_hidden(&mut self, on: bool) -> Result<(), String> {
        if self.show_hidden == on {
            return Ok(());
        }
        let previous = self.show_hidden;
        self.show_hidden = on;
        if let Err(err) = self.reload() {
            self.show_hidden = previous;
            return Err(err);
        }
        Ok(())
    }

    /// Re-read the root and every directory that is expanded. The previous
    /// nodes stay in place when a read fails.
    pub fn reload(&mut self) -> Result<(), String> {
        let expanded = expanded_canons(&self.nodes);
        let mut fresh = read_nodes(&self.root, self.show_hidden)?;
        let mut ancestors = vec![self.root.clone()];
        apply_expanded(
            &mut fresh,
            &expanded,
            &self.root,
            &mut ancestors,
            self.show_hidden,
        )?;
        self.nodes = fresh;
        Ok(())
    }

    pub fn flatten(&self) -> Vec<FlatRow> {
        let mut out = Vec::new();
        flatten_nodes(&self.nodes, 0, &mut out);
        out
    }

    pub fn toggle(&mut self, path: &Path) -> ExpandOutcome {
        let mut ancestors = vec![self.root.clone()];
        toggle_rec(
            &mut self.nodes,
            path,
            &self.root,
            &mut ancestors,
            self.show_hidden,
        )
        .unwrap_or(ExpandOutcome::NotADirectory)
    }
}

fn read_nodes(dir: &Path, show_hidden: bool) -> Result<Vec<TreeNode>, String> {
    let entries = read_children(dir, show_hidden).map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|entry| TreeNode {
            name: entry.name,
            path: entry.path,
            is_dir: entry.is_dir,
            expanded: false,
            children: Vec::new(),
        })
        .collect())
}

fn flatten_nodes(nodes: &[TreeNode], depth: usize, out: &mut Vec<FlatRow>) {
    for node in nodes {
        out.push(FlatRow {
            name: node.name.clone(),
            path: node.path.clone(),
            is_dir: node.is_dir,
            depth,
            expanded: node.expanded,
        });
        if node.expanded {
            flatten_nodes(&node.children, depth + 1, out);
        }
    }
}

fn expanded_canons(nodes: &[TreeNode]) -> HashSet<PathBuf> {
    let mut set = HashSet::new();
    fn walk(nodes: &[TreeNode], set: &mut HashSet<PathBuf>) {
        for node in nodes {
            if node.expanded {
                if let Ok(canon) = fs::canonicalize(&node.path) {
                    set.insert(canon);
                }
                walk(&node.children, set);
            }
        }
    }
    walk(nodes, &mut set);
    set
}

fn apply_expanded(
    nodes: &mut [TreeNode],
    expanded: &HashSet<PathBuf>,
    root: &Path,
    ancestors: &mut Vec<PathBuf>,
    show_hidden: bool,
) -> Result<(), String> {
    for node in nodes.iter_mut() {
        let canon = fs::canonicalize(&node.path).ok();
        let should = canon.as_ref().is_some_and(|path| expanded.contains(path));
        if should && node.is_dir {
            match expand_node(node, root, ancestors, show_hidden) {
                ExpandOutcome::Expanded => {}
                ExpandOutcome::Io(message) => return Err(message),
                ExpandOutcome::Collapsed
                | ExpandOutcome::OutsideRoot
                | ExpandOutcome::Loop
                | ExpandOutcome::NotADirectory => {}
            }
        }
        if node.expanded {
            if let Some(path) = canon {
                ancestors.push(path);
                apply_expanded(&mut node.children, expanded, root, ancestors, show_hidden)?;
                ancestors.pop();
            }
        }
    }
    Ok(())
}

fn toggle_rec(
    nodes: &mut [TreeNode],
    path: &Path,
    root: &Path,
    ancestors: &mut Vec<PathBuf>,
    show_hidden: bool,
) -> Option<ExpandOutcome> {
    for node in nodes.iter_mut() {
        if paths_match(&node.path, path) {
            return Some(expand_node(node, root, ancestors, show_hidden));
        }
        if node.expanded {
            let canon = fs::canonicalize(&node.path).ok();
            if let Some(path) = &canon {
                ancestors.push(path.clone());
            }
            let found = toggle_rec(&mut node.children, path, root, ancestors, show_hidden);
            if canon.is_some() {
                ancestors.pop();
            }
            if found.is_some() {
                return found;
            }
        }
    }
    None
}

fn expand_node(
    node: &mut TreeNode,
    root: &Path,
    ancestors: &[PathBuf],
    show_hidden: bool,
) -> ExpandOutcome {
    if !node.is_dir {
        return ExpandOutcome::NotADirectory;
    }
    if node.expanded {
        node.expanded = false;
        node.children.clear();
        return ExpandOutcome::Collapsed;
    }
    let canonical = match fs::canonicalize(&node.path) {
        Ok(path) => path,
        Err(err) => return ExpandOutcome::Io(err.to_string()),
    };
    if !canonical.starts_with(root) {
        return ExpandOutcome::OutsideRoot;
    }
    if ancestors.iter().any(|ancestor| ancestor == &canonical) {
        return ExpandOutcome::Loop;
    }
    match read_nodes(&node.path, show_hidden) {
        Ok(children) => {
            node.children = children;
            node.expanded = true;
            ExpandOutcome::Expanded
        }
        Err(message) => ExpandOutcome::Io(message),
    }
}

/// Row identity is the path we listed, not its canonical target.
/// A symlink and the directory it points at stay two rows.
fn paths_match(stored: &Path, query: &Path) -> bool {
    stored == query
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rust-edit-tree-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join("src/util.rs"), "").unwrap();
        fs::write(root.join("README.md"), "").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        std::os::unix::fs::symlink(root.join("src"), root.join("link")).unwrap();
        std::os::unix::fs::symlink("/tmp", root.join("outside")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("src/loop")).unwrap();
        root
    }

    fn names(rows: &[FlatRow]) -> Vec<&str> {
        rows.iter().map(|row| row.name.as_str()).collect()
    }

    #[test]
    fn load_hides_dotfiles_and_omits_parent() {
        let root = fixture();
        let tree = ProjectTree::load(&root, false).unwrap();
        assert!(!tree.show_hidden());
        let rows = tree.flatten();
        let listed = names(&rows);
        assert_eq!(listed, vec!["link", "outside", "src", "README.md"]);
        assert!(rows.iter().all(|row| row.depth == 0));
        assert!(rows.iter().all(|row| !row.path.ends_with("..")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn expand_then_collapse_src() {
        let root = fixture();
        let mut tree = ProjectTree::load(&root, false).unwrap();
        let src = tree
            .flatten()
            .into_iter()
            .find(|row| row.name == "src")
            .unwrap();
        assert_eq!(tree.toggle(&src.path), ExpandOutcome::Expanded);
        let rows = tree.flatten();
        let src_at = rows.iter().position(|row| row.name == "src").unwrap();
        assert_eq!(rows[src_at + 1].name, "loop");
        assert_eq!(rows[src_at + 1].depth, 1);
        assert_eq!(rows[src_at + 2].name, "main.rs");
        assert_eq!(rows[src_at + 2].depth, 1);
        assert_eq!(rows[src_at + 3].name, "util.rs");
        assert!(rows[src_at + 3].path.ends_with("util.rs"));
        assert_eq!(tree.toggle(&src.path), ExpandOutcome::Collapsed);
        assert!(!names(&tree.flatten()).contains(&"main.rs"));
        assert!(names(&tree.flatten()).contains(&"src"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reload_keeps_src_open_and_sees_a_new_file() {
        let root = fixture();
        let mut tree = ProjectTree::load(&root, false).unwrap();
        let src = tree
            .flatten()
            .into_iter()
            .find(|row| row.name == "src")
            .unwrap();
        tree.toggle(&src.path);
        fs::write(root.join("src/new.rs"), "").unwrap();
        tree.reload().unwrap();
        let rows = tree.flatten();
        let src_row = rows.iter().find(|row| row.name == "src").unwrap();
        assert!(src_row.expanded);
        assert!(names(&rows).contains(&"new.rs"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn hidden_toggle_reveals_git_and_keeps_expansion() {
        let root = fixture();
        let mut tree = ProjectTree::load(&root, false).unwrap();
        let src = tree
            .flatten()
            .into_iter()
            .find(|row| row.name == "src")
            .unwrap();
        tree.toggle(&src.path);
        tree.set_show_hidden(true).unwrap();
        let rows = tree.flatten();
        assert!(names(&rows).contains(&".git"));
        assert!(rows.iter().find(|row| row.name == "src").unwrap().expanded);
        assert!(names(&rows).contains(&"main.rs"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn outside_link_and_ancestor_loop_do_not_expand() {
        let root = fixture();
        let mut tree = ProjectTree::load(&root, false).unwrap();
        let outside = tree
            .flatten()
            .into_iter()
            .find(|row| row.name == "outside")
            .unwrap();
        assert_eq!(tree.toggle(&outside.path), ExpandOutcome::OutsideRoot);
        assert!(
            !tree
                .flatten()
                .iter()
                .find(|row| row.name == "outside")
                .unwrap()
                .expanded
        );

        let src = tree
            .flatten()
            .into_iter()
            .find(|row| row.name == "src")
            .unwrap();
        tree.toggle(&src.path);
        let loop_row = tree
            .flatten()
            .into_iter()
            .find(|row| row.name == "loop")
            .unwrap();
        assert_eq!(tree.toggle(&loop_row.path), ExpandOutcome::Loop);
        assert!(
            !tree
                .flatten()
                .iter()
                .find(|row| row.name == "loop")
                .unwrap()
                .expanded
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_directory_flattens_to_nothing() {
        let root = std::env::temp_dir().join(format!(
            "rust-edit-tree-empty-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let tree = ProjectTree::load(&root, false).unwrap();
        assert!(tree.flatten().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn toggling_a_file_is_not_a_directory() {
        let root = fixture();
        let mut tree = ProjectTree::load(&root, false).unwrap();
        let readme = tree
            .flatten()
            .into_iter()
            .find(|row| row.name == "README.md")
            .unwrap();
        assert_eq!(tree.toggle(&readme.path), ExpandOutcome::NotADirectory);
        let _ = fs::remove_dir_all(&root);
    }
}
