//! Persona-Based Code Generation
//!
//! Pure Data approach: Personas are JSON files loaded at runtime.
//! The Cortex selects the appropriate persona reflexively.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A Persona encodes the "Technical DNA" of an expert.
/// - `directives`: Positive rules (DO this)
/// - `constraints`: Negative rules (DO NOT do this)
/// - `language_mappings`: How to express concepts in specific languages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub id: String,
    pub name: String,
    pub domains: Vec<String>,
    pub directives: Vec<String>,
    pub constraints: Vec<String>,
    #[serde(default)]
    pub language_mappings: HashMap<String, HashMap<String, String>>,
}

impl Persona {
    /// Generates the prompt fragment for this persona, specialized for a target language.
    pub fn to_prompt(&self, target_lang: Option<&str>) -> String {
        let mut prompt = format!("### Active Persona: {}\n\n", self.name);

        prompt.push_str("**Directives (DO):**\n");
        for d in &self.directives {
            prompt.push_str(&format!("- {}\n", d));
        }

        prompt.push_str("\n**Constraints (DO NOT):**\n");
        for c in &self.constraints {
            prompt.push_str(&format!("- {}\n", c));
        }

        if let Some(lang) = target_lang {
            if let Some(lang_map) = self.language_mappings.get(lang) {
                prompt.push_str(&format!("\n**{} Specifics:**\n", lang.to_uppercase()));
                for (concept, guidance) in lang_map {
                    prompt.push_str(&format!("- *{}*: {}\n", concept, guidance));
                }
            }
        }

        prompt
    }
}

/// Load all personas from a directory of JSON files.
pub fn load_personas(personas_dir: &Path) -> std::io::Result<Vec<Persona>> {
    let mut personas = Vec::new();

    if !personas_dir.exists() {
        return Ok(personas); // No personas dir = no personas
    }

    for entry in std::fs::read_dir(personas_dir)? {
        let path = entry?.path();
        if path.extension().map_or(false, |e| e == "json") {
            let content = std::fs::read_to_string(&path)?;
            match serde_json::from_str::<Persona>(&content) {
                Ok(persona) => personas.push(persona),
                Err(e) => {
                    eprintln!("⚠️  Failed to parse persona {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(personas)
}

/// Find a persona by ID.
pub fn find_persona<'a>(personas: &'a [Persona], id: &str) -> Option<&'a Persona> {
    personas.iter().find(|p| p.id == id.trim())
}

/// The prompt used to ask the model to select a persona.
pub const PERSONA_SELECTOR_PROMPT: &str = r#"You have access to the following expert personas. Analyze the user's request and select the MOST appropriate persona ID. Output ONLY the persona ID on a single line, nothing else.

Available Personas:
- hickey: Architecture, simplicity, state management, concurrency, data-orientation.
- sierra: Large-scale systems, lifecycle management, dependency injection, testing.
- nolen: UI/Frontend, immutable state, reactive rendering, single source of truth.
- halloway: Debugging, scientific method, generative testing, reproducibility.
- steele: Parallelism, associativity, monoids, high-performance computing.
- kay: Biological systems, message-passing, late binding, actors.

User Request: {user_request}

Selected Persona ID:"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_persona_to_prompt() {
        let persona = Persona {
            id: "test".into(),
            name: "Test Persona".into(),
            domains: vec!["testing".into()],
            directives: vec!["Be simple".into()],
            constraints: vec!["No complexity".into()],
            language_mappings: {
                let mut m = HashMap::new();
                let mut rust = HashMap::new();
                rust.insert("style".into(), "Use match expressions".into());
                m.insert("rust".into(), rust);
                m
            },
        };

        let prompt = persona.to_prompt(Some("rust"));
        assert!(prompt.contains("Test Persona"));
        assert!(prompt.contains("Be simple"));
        assert!(prompt.contains("No complexity"));
        assert!(prompt.contains("RUST Specifics"));
        assert!(prompt.contains("Use match expressions"));
    }

    #[test]
    fn test_load_personas_from_sly() {
        // This test assumes `.sly/personas/` exists with persona files
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".sly/personas");
        if dir.exists() {
            let personas = load_personas(&dir).unwrap();
            assert!(!personas.is_empty(), "Should load at least one persona");

            // Verify hickey exists
            let hickey = find_persona(&personas, "hickey");
            assert!(hickey.is_some(), "Should find hickey persona");
        }
    }
}
