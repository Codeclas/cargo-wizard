use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use toml_edit::{table, value, Document, Item};

use crate::toml::BuiltinProfile;
use crate::TomlProfileTemplate;

#[derive(Debug)]
pub struct ParsedProfile {
    name: String,
    items: HashMap<String, Item>,
}

#[derive(Debug)]
pub struct ParsedManifest {
    document: Document,
    profiles: HashMap<String, ParsedProfile>,
}

impl ParsedManifest {
    pub fn apply_profile(
        mut self,
        name: &str,
        template: TomlProfileTemplate,
    ) -> anyhow::Result<Self> {
        let profiles_table = self
            .document
            .entry("profile")
            .or_insert(table())
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("The profile item in Cargo.toml is not a table"))?;
        profiles_table.set_dotted(true);

        let profile_table = profiles_table
            .entry(name)
            .or_insert(table())
            .as_table_mut()
            .ok_or_else(|| {
                anyhow::anyhow!("The profile.{name} table in Cargo.toml is not a table")
            })?;

        if !is_builtin_profile(name) {
            let inherits = match template.inherits {
                BuiltinProfile::Dev => "dev",
                BuiltinProfile::Release => "release",
            };

            // Add "inherits" as the first key of the table
            let items: Vec<_> = profile_table
                .iter()
                .map(|(n, i)| (n.to_string(), i.clone()))
                .collect();
            profile_table.clear();
            if !items.iter().any(|(name, _)| *name == "inherits") {
                profile_table.insert("inherits", value(inherits));
            }
            for (name, item) in items {
                profile_table.insert(&name, item);
            }
        }

        for (key, val) in &template.template.fields {
            let mut new_value = val.to_toml_value();

            if let Some(existing_item) = profile_table.get_mut(key) {
                if let Some(value) = existing_item.as_value() {
                    *new_value.decor_mut() = value.decor().clone();
                }
                *existing_item = value(new_value);
            } else {
                profile_table.insert(key, value(new_value));
            }
        }

        Ok(self)
    }

    pub fn write(self, path: &Path) -> anyhow::Result<()> {
        std::fs::write(path, self.document.to_string())?;
        Ok(())
    }
}

fn is_builtin_profile(name: &str) -> bool {
    matches!(name, "dev" | "release")
}

pub fn parse_manifest(path: &Path) -> anyhow::Result<ParsedManifest> {
    let manifest = std::fs::read_to_string(path).context("Cannot read Cargo.toml manifest")?;
    let manifest = manifest
        .parse::<Document>()
        .context("Cannot parse Cargo.toml manifest")?;

    let profiles = if let Some(profiles) = manifest.get("profile").and_then(|p| p.as_table_like()) {
        profiles
            .iter()
            .filter_map(|(name, table)| table.as_table().map(|t| (name, t)))
            .map(|(name, table)| {
                let name = name.to_string();

                let items = table
                    .iter()
                    .map(|(name, item)| (name.to_string(), item.clone()))
                    .collect();

                let profile = ParsedProfile {
                    name: name.clone(),
                    items,
                };

                (name, profile)
            })
            .collect()
    } else {
        Default::default()
    };
    Ok(ParsedManifest {
        profiles,
        document: manifest,
    })
}
