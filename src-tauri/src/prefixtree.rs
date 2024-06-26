use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// This is quite a bit slower than what it can be, but it makes writing the api for it much easier
/// and I couldn't find a library that does it, really wasted too much time on this.
pub struct PrefixTree {
    #[serde(flatten)]
    pub root: HashMap<String, PrefixTree>,
}

impl PrefixTree {
    pub fn new() -> Self {
        Self {
            root: HashMap::new(),
        }
    }

    /// quickly build the prefix tree also often called a "trie"
    pub fn insert(&mut self, path: &std::path::Path) {
        let mut tree = self;
        for component in path.components() {
            tree = tree
                .root
                .entry(component.as_os_str().to_string_lossy().to_string())
                .or_insert_with(Default::default);
        }
    }
}

#[test]
fn test_prefix_tree() {
    let mut tree = PrefixTree::new();
    tree.insert(std::path::Path::new("a/b/c"));
    tree.insert(std::path::Path::new("a/b/c/d"));
    tree.insert(std::path::Path::new("a/b/c/e"));
    tree.insert(std::path::Path::new("a/b/f"));
    tree.insert(std::path::Path::new("u/b/f"));
    // this exists purely for debugging json output
    println!("{}", serde_json::to_string_pretty(&tree).unwrap());
}
