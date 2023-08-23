use regex::Regex;

#[derive(Debug)]
pub struct Rule {
    pub pattern: Regex,
    pub label_name: String,
    pub label_value: String,
}
