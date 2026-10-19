//! Playlist folders use names, so every move works across all server backends.
use crate::model::Item;
use anyhow::{Result, ensure};

#[derive(Clone, Debug)]
pub enum Node {
    Folder {
        name: String,
        path: String,
        children: Vec<Node>,
    },
    Playlist {
        name: String,
        item: Box<Item>,
    },
}
pub fn order(nodes: &mut [Node], ids: &[String]) {
    let ranks: std::collections::HashMap<_, _> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect();
    fn rank(node: &Node, ranks: &std::collections::HashMap<&str, usize>) -> usize {
        match node {
            Node::Playlist { item, .. } => {
                ranks.get(item.id.as_str()).copied().unwrap_or(usize::MAX)
            }
            Node::Folder { children, .. } => children
                .iter()
                .map(|node| rank(node, ranks))
                .min()
                .unwrap_or(usize::MAX),
        }
    }
    fn apply(nodes: &mut [Node], ranks: &std::collections::HashMap<&str, usize>) {
        for node in nodes.iter_mut() {
            if let Node::Folder { children, .. } = node {
                apply(children, ranks);
            }
        }
        nodes.sort_by_key(|node| rank(node, ranks));
    }
    apply(nodes, &ranks);
}
pub fn leaf<'a>(name: &'a str, separator: &str) -> &'a str {
    if separator.is_empty() {
        return name;
    }
    name.rsplit(separator)
        .find(|part| !part.is_empty())
        .unwrap_or(name)
}
pub fn in_folder(name: &str, folder: &str, separator: &str) -> bool {
    !folder.is_empty() && !separator.is_empty() && name.starts_with(&format!("{folder}{separator}"))
}
pub fn move_playlist(name: &str, target: &str, separator: &str) -> String {
    let leaf = leaf(name, separator).trim();
    if target.is_empty() {
        leaf.into()
    } else {
        format!("{target}{separator}{leaf}")
    }
}
pub fn move_folder(
    name: &str,
    source: &str,
    target: &str,
    separator: &str,
) -> Result<Option<String>> {
    ensure!(
        !separator.is_empty() && !source.is_empty(),
        "Choose a playlist folder"
    );
    ensure!(
        source != target && !in_folder(target, source, separator),
        "A folder cannot move into itself or its descendants"
    );
    let Some(remainder) = name
        .strip_prefix(&format!("{source}{separator}"))
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if target.is_empty() {
        return Ok(Some(
            if source
                .split(separator)
                .filter(|part| !part.is_empty())
                .count()
                == 1
            {
                leaf(name, separator).trim().into()
            } else {
                format!("{}{separator}{remainder}", leaf(source, separator))
            },
        ));
    }
    if in_folder(source, target, separator) {
        let relative = source
            .strip_prefix(&format!("{target}{separator}"))
            .unwrap();
        return Ok(Some(if !relative.contains(separator) {
            format!("{target}{separator}{remainder}")
        } else {
            format!(
                "{target}{separator}{}{separator}{remainder}",
                leaf(source, separator)
            )
        }));
    }
    Ok(Some(format!(
        "{target}{separator}{source}{separator}{remainder}"
    )))
}
pub fn tree(items: &[Item], separator: &str, single: bool) -> Vec<Node> {
    fn insert(nodes: &mut Vec<Node>, path: &str, parts: &[&str], item: &Item, separator: &str) {
        if parts.len() <= 1 {
            nodes.push(Node::Playlist {
                name: parts.first().copied().unwrap_or(&item.name).into(),
                item: Box::new(item.clone()),
            });
            return;
        }
        let name = parts[0];
        let folder = if path.is_empty() {
            name.to_owned()
        } else {
            format!("{path}{separator}{name}")
        };
        let index = nodes
            .iter()
            .position(|node| matches!(node, Node::Folder { path, .. } if path == &folder))
            .unwrap_or_else(|| {
                nodes.push(Node::Folder {
                    name: name.into(),
                    path: folder.clone(),
                    children: vec![],
                });
                nodes.len() - 1
            });
        if let Node::Folder { children, .. } = &mut nodes[index] {
            insert(children, &folder, &parts[1..], item, separator);
        }
    }
    fn folders_first(nodes: &mut [Node]) {
        for node in nodes.iter_mut() {
            if let Node::Folder { children, .. } = node {
                folders_first(children);
            }
        }
        nodes.sort_by_key(|node| matches!(node, Node::Playlist { .. }));
    }
    let mut nodes = vec![];
    for item in items {
        let parts = if separator.is_empty() {
            vec![item.name.as_str()]
        } else if single {
            item.name
                .split_once(separator)
                .filter(|(left, right)| !left.is_empty() && !right.is_empty())
                .map_or(vec![item.name.as_str()], |(left, right)| vec![left, right])
        } else {
            item.name
                .splitn(65, separator)
                .filter(|part| !part.is_empty())
                .collect()
        };
        if parts.len() <= 1 {
            nodes.push(Node::Playlist {
                name: item.name.clone(),
                item: Box::new(item.clone()),
            });
        } else {
            insert(&mut nodes, "", &parts, item, separator);
        }
    }
    folders_first(&mut nodes);
    nodes
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folder_moves_match_parent_ancestor_root_and_descendant_rules() {
        for (name, source, target, expected) in [
            ("Rock/Classic/x", "Rock/Classic", "Rock", "Rock/x"),
            (
                "Pop/Rock/Classic/x",
                "Pop/Rock/Classic",
                "Pop",
                "Pop/Classic/x",
            ),
            ("Classic/x", "Classic", "Rock", "Rock/Classic/x"),
            ("Rock/Sub/x", "Rock", "", "x"),
            ("Auto/Test/x", "Auto/Test", "", "Test/x"),
        ] {
            assert_eq!(
                move_folder(name, source, target, "/").unwrap(),
                Some(expected.into())
            );
        }
        assert!(move_folder("Rock/x", "Rock", "Rock/Sub", "/").is_err());
        assert!(move_folder("Rock/x", "Rock", "Rock", "/").is_err());
        assert_eq!(move_folder("Other/x", "Rock", "Pop", "/").unwrap(), None);
        assert_eq!(move_playlist("Rock::Live::x", "Jazz", "::"), "Jazz::x");
        let items = ["root", "Rock/Classic/x", "Rock/y", "/Pop//z"].map(|name| Item {
            name: name.into(),
            ..Default::default()
        });
        let nested = tree(&items, "/", false);
        let Node::Folder { path, children, .. } = &nested[0] else {
            panic!()
        };
        assert_eq!(path, "Rock");
        assert!(matches!(children[0], Node::Folder { .. }));
        let flat = tree(&items, "/", true);
        let Node::Folder { children, .. } = &flat[0] else {
            panic!()
        };
        assert!(matches!(&children[0], Node::Playlist { name, .. } if name == "Classic/x"));
        assert!(matches!(nested.last(), Some(Node::Playlist { name, .. }) if name == "root"));
    }
}
