use crate::config::RuleGroup;
use mtop_client::Meta;

#[derive(Debug)]
pub struct LabelParser<'a> {
    config: &'a RuleGroup,
}

impl<'a> LabelParser<'a> {
    pub fn new(config: &'a RuleGroup) -> Self {
        Self { config }
    }

    pub fn extract(&self, meta: &Meta) -> Vec<(String, String)> {
        // Using a Vec here instead of a HashSet because checking for inclusion
        // in a vector is faster when the number of entries is small. The number
        // of label names should be small since the correspond to labels added to
        // Prometheus metrics, should be single digits.
        let mut names = Vec::new();
        let mut labels = Vec::new();
        let mut value = String::new();
        for rule in self.config.rules.iter() {
            if names.contains(&&rule.label_name) {
                continue;
            }

            if let Some(c) = rule.pattern.captures(&meta.key) {
                names.push(&rule.label_name);

                value.clear();
                c.expand(&rule.label_value, &mut value);
                labels.push((rule.label_name.clone(), value.clone()));
            }
        }

        labels
    }
}
