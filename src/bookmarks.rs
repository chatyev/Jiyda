//! Закладки: дерево папок и ссылок (bookmarks.json).
//!
//! Закладки — постоянная библиотека сайтов, отдельно от открытых страниц (Now, Later,
//! Pinned): их ищут адресная строка и палитра, показывает панель «Закладки»
//! (Ctrl+Shift+O), сюда же переносятся закладки Chrome и Edge (import.rs).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Папка или ссылка. Папка — у кого есть `children` (даже пустой список).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Node {
    pub id: u64,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Node>>,
    /// Когда добавлена (секунды Unix; 0 — неизвестно).
    #[serde(skip_serializing_if = "is_zero")]
    pub added: u64,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

impl Node {
    pub fn link(title: &str, url: &str, added: u64) -> Self {
        Self { id: 0, title: title.into(), url: url.into(), children: None, added }
    }
    pub fn folder(title: &str, children: Vec<Node>) -> Self {
        Self { id: 0, title: title.into(), url: String::new(), children: Some(children), added: 0 }
    }
    pub fn is_folder(&self) -> bool {
        self.children.is_some()
    }
    /// Ссылок внутри (с вложенными папками); у ссылки — 1.
    pub fn count_links(&self) -> usize {
        match &self.children {
            Some(c) => c.iter().map(Node::count_links).sum(),
            None => 1,
        }
    }
}

/// Удалённая закладка или папка и где она была — для Undo.
#[derive(Debug, Clone)]
pub struct Removed {
    pub node: Node,
    pub parent: Option<u64>,
    pub index: usize,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    items: Vec<Node>,
}

pub struct Bookmarks {
    /// Верхний уровень (корень сам не хранится).
    items: Vec<Node>,
    next_id: u64,
    /// Адреса всех ссылок — для звёздочки в адресной строке.
    urls: HashSet<String>,
    dirty: bool,
}

/// Сколько закладок держим всего: импорт огромных коллекций дальше не идёт.
pub const LIMIT: usize = 20_000;

impl Bookmarks {
    pub fn load(path: &Path) -> Self {
        let file: File =
            std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self::from_items(file.items)
    }

    fn from_items(items: Vec<Node>) -> Self {
        let mut b = Self { items, next_id: 1, urls: HashSet::new(), dirty: false };
        // Номера должны быть уникальны: старые файлы и ручные правки чиним при загрузке.
        let mut seen = HashSet::new();
        let mut broken = false;
        walk(&b.items, &mut |n| broken |= n.id == 0 || !seen.insert(n.id));
        if broken {
            let mut next = 1;
            walk_mut(&mut b.items, &mut |n| {
                n.id = next;
                next += 1;
            });
        }
        let mut max = 0;
        walk(&b.items, &mut |n| max = max.max(n.id));
        b.next_id = max + 1;
        b.reindex();
        b
    }

    /// Для записи в фоне: данные, если были изменения, и отметка «записано».
    pub fn take_changes(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        serde_json::to_string(&File { items: self.items.clone() }).ok()
    }

    /// Синхронная запись — для тестов; браузер пишет через `config::Writer`.
    #[cfg(test)]
    pub fn save(&mut self, path: &Path) {
        if let Some(s) = self.take_changes() {
            crate::config::write_atomic(path, &s);
        }
    }

    pub fn items(&self) -> &[Node] {
        &self.items
    }

    pub fn contains(&self, url: &str) -> bool {
        self.urls.contains(url)
    }

    pub fn len(&self) -> usize {
        self.urls.len()
    }

    /// Первая закладка с этим адресом.
    pub fn find_url(&self, url: &str) -> Option<u64> {
        let mut found = None;
        walk(&self.items, &mut |n| {
            if found.is_none() && !n.is_folder() && n.url == url {
                found = Some(n.id);
            }
        });
        found
    }

    pub fn get(&self, id: u64) -> Option<&Node> {
        find(&self.items, id)
    }

    /// Добавить ссылку в папку (None — верхний уровень), в конец.
    pub fn add(&mut self, parent: Option<u64>, title: &str, url: &str, added: u64) -> Option<u64> {
        self.insert(parent, Node::link(title, url, added))
    }

    pub fn add_folder(&mut self, parent: Option<u64>, title: &str) -> Option<u64> {
        self.insert(parent, Node::folder(title, Vec::new()))
    }

    fn insert(&mut self, parent: Option<u64>, mut node: Node) -> Option<u64> {
        self.assign_ids(&mut node);
        let id = node.id;
        self.children_mut(parent)?.push(node);
        self.changed();
        Some(id)
    }

    fn assign_ids(&mut self, node: &mut Node) {
        node.id = self.next_id;
        self.next_id += 1;
        if let Some(c) = &mut node.children {
            for n in c {
                self.assign_ids(n);
            }
        }
    }

    /// Список детей папки (None — верхний уровень); None — такой папки нет.
    fn children_mut(&mut self, parent: Option<u64>) -> Option<&mut Vec<Node>> {
        match parent {
            None => Some(&mut self.items),
            Some(id) => find_mut(&mut self.items, id)?.children.as_mut(),
        }
    }

    pub fn remove(&mut self, id: u64) -> Option<Removed> {
        let (parent, index) = locate(&self.items, None, id)?;
        let node = self.children_mut(parent)?.remove(index);
        self.changed();
        Some(Removed { node, parent, index })
    }

    /// Вернуть удалённое (Undo) на прежнее место; папки уже нет — на верхний уровень.
    pub fn restore(&mut self, r: Removed) {
        let parent = r.parent.filter(|p| self.get(*p).is_some_and(Node::is_folder));
        let list = match self.children_mut(parent) {
            Some(l) => l,
            None => &mut self.items,
        };
        let index = if parent == r.parent { r.index.min(list.len()) } else { list.len() };
        list.insert(index, r.node);
        self.changed();
    }

    /// Сохранить страницу (окно Ctrl+D): адрес уже в закладках — второй не появляется,
    /// у существующей меняются название и папка. Папки `parent` нет — на верхний уровень.
    /// Возвращает номер закладки и была ли она новой.
    pub fn save_page(&mut self, parent: Option<u64>, title: &str, url: &str, added: u64) -> (u64, bool) {
        let parent = parent.filter(|p| self.get(*p).is_some_and(Node::is_folder));
        let title = match title.trim() {
            "" => url,
            t => t,
        };
        if let Some(id) = self.find_url(url) {
            self.edit(id, title, None);
            if locate(&self.items, None, id).is_some_and(|(p, _)| p != parent) {
                self.move_to(id, parent, None);
            }
            return (id, false);
        }
        let id = self.add(parent, title, url, added).expect("папка проверена выше");
        (id, true)
    }

    /// Изменить название и (у ссылки) адрес.
    pub fn edit(&mut self, id: u64, title: &str, url: Option<&str>) -> bool {
        let Some(n) = find_mut(&mut self.items, id) else { return false };
        n.title = title.trim().to_string();
        if let Some(u) = url.filter(|_| !n.is_folder()).map(str::trim).filter(|u| !u.is_empty()) {
            n.url = u.to_string();
        }
        self.changed();
        true
    }

    /// Перенести в папку `parent` (None — верхний уровень) на место `index` (None — в конец).
    /// Папку нельзя перенести в саму себя или в свою вложенную папку.
    pub fn move_to(&mut self, id: u64, parent: Option<u64>, index: Option<usize>) -> bool {
        if let Some(p) = parent {
            let Some(target) = self.get(p) else { return false };
            if !target.is_folder()
                || p == id
                || self.get(id).is_some_and(|n| find(n.children.as_deref().unwrap_or(&[]), p).is_some())
            {
                return false;
            }
        }
        let Some((from_parent, from_index)) = locate(&self.items, None, id) else { return false };
        let Some(node) = self.children_mut(from_parent).map(|l| l.remove(from_index)) else { return false };
        let list = self.children_mut(parent).expect("папка проверена выше");
        let mut at = index.unwrap_or(list.len());
        // В той же папке ниже прежнего места: после удаления места сдвинулись на одно.
        if from_parent == parent && index.is_some() && at > from_index {
            at -= 1;
        }
        list.insert(at.min(list.len()), node);
        self.changed();
        true
    }

    /// Перенести закладки из другого браузера папкой `title` на верхний уровень. Адреса,
    /// которые уже есть в закладках, пропускаются (повторный импорт ничего не удваивает);
    /// опустевшие папки — тоже. Возвращает, сколько ссылок добавлено.
    pub fn import(&mut self, title: &str, nodes: Vec<Node>) -> usize {
        let mut seen = self.urls.clone();
        let mut room = LIMIT.saturating_sub(self.len());
        let nodes = dedup(nodes, &mut seen, &mut room);
        let added: usize = nodes.iter().map(Node::count_links).sum();
        if added == 0 {
            return 0;
        }
        // Папка с таким именем уже есть (прошлый импорт) — дополняем её.
        let existing = self.items.iter().find(|n| n.is_folder() && n.title == title).map(|n| n.id);
        let folder = match existing {
            Some(id) => id,
            None => match self.add_folder(None, title) {
                Some(id) => id,
                None => return 0,
            },
        };
        let mut nodes = nodes;
        for n in &mut nodes {
            self.assign_ids(n);
        }
        if let Some(list) = self.children_mut(Some(folder)) {
            list.extend(nodes);
        }
        self.changed();
        added
    }

    fn changed(&mut self) {
        self.reindex();
        self.dirty = true;
    }

    fn reindex(&mut self) {
        let mut urls = HashSet::new();
        walk(&self.items, &mut |n| {
            if !n.is_folder() {
                urls.insert(n.url.clone());
            }
        });
        self.urls = urls;
    }
}

fn dedup(nodes: Vec<Node>, seen: &mut HashSet<String>, room: &mut usize) -> Vec<Node> {
    let mut out = Vec::new();
    for mut n in nodes {
        match n.children.take() {
            Some(c) => {
                let c = dedup(c, seen, room);
                if !c.is_empty() {
                    n.children = Some(c);
                    out.push(n);
                }
            }
            None => {
                if *room > 0 && seen.insert(n.url.clone()) {
                    *room -= 1;
                    out.push(n);
                }
            }
        }
    }
    out
}

fn walk(nodes: &[Node], f: &mut impl FnMut(&Node)) {
    for n in nodes {
        f(n);
        if let Some(c) = &n.children {
            walk(c, f);
        }
    }
}

fn walk_mut(nodes: &mut [Node], f: &mut impl FnMut(&mut Node)) {
    for n in nodes {
        f(n);
        if let Some(c) = &mut n.children {
            walk_mut(c, f);
        }
    }
}

fn find(nodes: &[Node], id: u64) -> Option<&Node> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(found) = n.children.as_deref().and_then(|c| find(c, id)) {
            return Some(found);
        }
    }
    None
}

fn find_mut(nodes: &mut [Node], id: u64) -> Option<&mut Node> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(found) = n.children.as_deref_mut().and_then(|c| find_mut(c, id)) {
            return Some(found);
        }
    }
    None
}

/// Где лежит узел: (папка, индекс в ней).
fn locate(nodes: &[Node], parent: Option<u64>, id: u64) -> Option<(Option<u64>, usize)> {
    for (i, n) in nodes.iter().enumerate() {
        if n.id == id {
            return Some((parent, i));
        }
        if let Some(found) = n.children.as_deref().and_then(|c| locate(c, Some(n.id), id)) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
#[path = "../tests/unit/bookmarks.rs"]
mod tests;
