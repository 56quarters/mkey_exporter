use crate::config::Rule;
use mtop_client::Meta;
use std::collections::HashSet;

#[derive(Debug)]
pub struct LabelParser<'a> {
    config: &'a [Rule],
    value_cache: String,
    names_cache: HashSet<String>,
}

impl<'a> LabelParser<'a> {
    pub fn new(config: &'a [Rule]) -> Self {
        Self {
            config,
            value_cache: String::new(),
            names_cache: HashSet::new(),
        }
    }

    pub fn extract(&mut self, meta: &Meta) -> Vec<(String, String)> {
        self.value_cache.clear();
        self.names_cache.clear();

        let mut labels = Vec::new();

        for rule in self.config.iter() {
            if self.names_cache.contains(&rule.label_name) {
                continue;
            }

            if let Some(c) = rule.pattern.captures(&meta.key) {
                self.names_cache.insert(rule.label_name.clone());

                c.expand(&rule.label_value, &mut self.value_cache);
                labels.push((rule.label_name.clone(), self.value_cache.clone()));
                self.value_cache.clear();
            }
        }

        labels
    }
}
