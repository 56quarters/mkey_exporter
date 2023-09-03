use crate::config::RuleGroup;
use mtop_client::Meta;
use std::collections::HashSet;

#[derive(Debug)]
pub struct LabelParser<'a> {
    config: &'a RuleGroup,
}

impl<'a> LabelParser<'a> {
    pub fn new(config: &'a RuleGroup) -> Self {
        Self { config }
    }

    pub fn extract(&self, meta: &Meta) -> Vec<(String, String)> {
        let mut names_seen = HashSet::new();
        let mut value = String::new();
        let mut labels = Vec::new();
        for rule in self.config.rules.iter() {
            if names_seen.contains(&rule.label_name) {
                continue;
            }

            if let Some(c) = rule.pattern.captures(&meta.key) {
                names_seen.insert(&rule.label_name);

                c.expand(&rule.label_value, &mut value);
                labels.push((rule.label_name.clone(), value.clone()));
                value.clear();
            }
        }

        labels
    }
}
