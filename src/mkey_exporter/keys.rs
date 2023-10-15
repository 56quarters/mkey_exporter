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

#[cfg(test)]
mod test {
    use super::LabelParser;
    use crate::config::{Rule, RuleGroup, RulePattern};
    use mtop_client::Meta;

    fn new_meta(key: &str) -> Meta {
        Meta {
            key: key.to_owned(),
            ..Default::default()
        }
    }

    fn user_rule() -> Rule {
        Rule {
            pattern: RulePattern::new(r"\w+:([\w-]+):").unwrap(),
            label_name: "user".to_owned(),
            label_value: "u$1".to_owned(),
        }
    }

    fn type_rule() -> Rule {
        Rule {
            pattern: RulePattern::new(r"([\w-]+):\w+:").unwrap(),
            label_name: "type".to_owned(),
            label_value: "$1".to_owned(),
        }
    }

    fn specific_type_rules() -> Vec<Rule> {
        vec![
            Rule {
                pattern: RulePattern::new(r"u-c:\w+:").unwrap(),
                label_name: "type".to_owned(),
                label_value: "cart".to_owned(),
            },
            Rule {
                pattern: RulePattern::new(r"u-p:\w+:").unwrap(),
                label_name: "type".to_owned(),
                label_value: "profile".to_owned(),
            },
            Rule {
                pattern: RulePattern::new(r"([\w-]+):\w+:").unwrap(),
                label_name: "type".to_owned(),
                label_value: "unknown".to_owned(),
            },
        ]
    }

    #[test]
    fn test_extract_single_label() {
        let meta = new_meta("u-p:12345:something");
        let group = RuleGroup {
            name: "test".to_owned(),
            rules: vec![user_rule()],
        };

        let parser = LabelParser::new(&group);
        let labels = parser.extract(&meta);

        assert_eq!(vec![("user".to_owned(), "u12345".to_owned())], labels);
    }

    #[test]
    fn test_extract_multiple_labels() {
        let meta = new_meta("u-p:12345:something");
        let group = RuleGroup {
            name: "test".to_owned(),
            rules: vec![user_rule(), type_rule()],
        };

        let parser = LabelParser::new(&group);
        let labels = parser.extract(&meta);

        assert_eq!(
            vec![
                ("user".to_owned(), "u12345".to_owned()),
                ("type".to_owned(), "u-p".to_owned()),
            ],
            labels
        );
    }

    #[test]
    fn test_extract_multiple_labels_precedence() {
        let meta1 = new_meta("u-p:123:something");
        let meta2 = new_meta("u-c:456:something");
        let meta3 = new_meta("u-v:789:something");

        let mut rules = Vec::new();
        rules.push(user_rule());
        rules.extend(specific_type_rules());

        let group = RuleGroup {
            name: "test".to_owned(),
            rules,
        };

        let parser = LabelParser::new(&group);
        let labels1 = parser.extract(&meta1);
        let labels2 = parser.extract(&meta2);
        let labels3 = parser.extract(&meta3);

        assert_eq!(
            vec![
                ("user".to_owned(), "u123".to_owned()),
                ("type".to_owned(), "profile".to_owned()),
            ],
            labels1
        );
        assert_eq!(
            vec![
                ("user".to_owned(), "u456".to_owned()),
                ("type".to_owned(), "cart".to_owned()),
            ],
            labels2
        );
        assert_eq!(
            vec![
                ("user".to_owned(), "u789".to_owned()),
                ("type".to_owned(), "unknown".to_owned()),
            ],
            labels3
        );
    }
}
