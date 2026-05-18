use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Person,
    Project,
    File,
    Tag,
    Url,
    Code,
    Concept,
}

impl EntityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Project => "project",
            Self::File => "file",
            Self::Tag => "tag",
            Self::Url => "url",
            Self::Code => "code",
            Self::Concept => "concept",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityLink {
    pub entity: Entity,
    pub scope: String,
    pub memory_id: String,
    pub memory_summary: String,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityGraph {
    pub scope: Option<String>,
    pub entities: Vec<EntityNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityNode {
    pub entity: Entity,
    pub memories: usize,
    pub weight: f32,
}

pub fn extract_entities(text: &str) -> Vec<Entity> {
    let mut entities = BTreeSet::new();

    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|ch: char| {
            matches!(
                ch,
                ',' | '.' | ';' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\''
            )
        });

        if token.len() < 2 {
            continue;
        }

        if token.starts_with("http://") || token.starts_with("https://") {
            entities.insert(Entity {
                name: token.to_string(),
                kind: EntityKind::Url,
            });
            continue;
        }

        if let Some(tag) = token.strip_prefix('#') {
            if tag.len() >= 2 {
                entities.insert(Entity {
                    name: tag.to_ascii_lowercase(),
                    kind: EntityKind::Tag,
                });
            }
            continue;
        }

        if looks_like_file(token) {
            entities.insert(Entity {
                name: token.to_string(),
                kind: EntityKind::File,
            });
            continue;
        }

        if looks_like_code(token) {
            entities.insert(Entity {
                name: token.to_string(),
                kind: EntityKind::Code,
            });
            continue;
        }

        if looks_like_project(token) {
            entities.insert(Entity {
                name: token.to_string(),
                kind: EntityKind::Project,
            });
            continue;
        }
    }

    for phrase in capitalized_phrases(text) {
        entities.insert(Entity {
            name: phrase,
            kind: EntityKind::Concept,
        });
    }

    entities.into_iter().take(96).collect()
}

pub fn summarize_links(scope: Option<String>, links: Vec<EntityLink>) -> EntityGraph {
    let mut by_entity: BTreeMap<Entity, (usize, f32)> = BTreeMap::new();

    for link in links {
        let entry = by_entity.entry(link.entity).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += link.weight;
    }

    let mut entities = by_entity
        .into_iter()
        .map(|(entity, (memories, weight))| EntityNode {
            entity,
            memories,
            weight,
        })
        .collect::<Vec<_>>();

    entities.sort_by(|left, right| {
        right
            .weight
            .partial_cmp(&left.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.memories.cmp(&left.memories))
    });

    EntityGraph { scope, entities }
}

fn looks_like_file(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.ends_with(".rs")
        || lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".py")
        || lower.ends_with(".cpp")
        || lower.ends_with(".c")
        || lower.ends_with(".h")
        || lower.ends_with(".md")
        || lower.ends_with(".json")
        || lower.contains('\\')
        || lower.contains('/')
}

fn looks_like_code(token: &str) -> bool {
    token.contains("::")
        || token.contains("()")
        || token.contains("->")
        || token.contains("=>")
        || token.contains('_')
}

fn looks_like_project(token: &str) -> bool {
    token.contains(".cpp")
        || token.contains("-")
        || token.eq_ignore_ascii_case("ollama")
        || token.eq_ignore_ascii_case("llama.cpp")
}

fn capitalized_phrases(text: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    let mut current = Vec::new();

    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
        if token.len() < 3 {
            flush_phrase(&mut phrases, &mut current);
            continue;
        }

        let starts_upper = token
            .chars()
            .next()
            .map(|ch| ch.is_ascii_uppercase())
            .unwrap_or(false);

        if starts_upper {
            current.push(token.to_string());
        } else {
            flush_phrase(&mut phrases, &mut current);
        }
    }

    flush_phrase(&mut phrases, &mut current);
    phrases
}

fn flush_phrase(phrases: &mut Vec<String>, current: &mut Vec<String>) {
    if current.is_empty() {
        return;
    }

    let phrase = current.join(" ");
    if phrase.len() >= 3 && !is_common_sentence_start(&phrase) {
        phrases.push(phrase);
    }

    current.clear();
}

fn is_common_sentence_start(value: &str) -> bool {
    matches!(
        value,
        "The" | "This" | "That" | "When" | "Where" | "Why" | "How" | "Use" | "Add"
    )
}
