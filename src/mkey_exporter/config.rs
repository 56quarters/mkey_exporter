use regex::Regex;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fs::File;
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

pub fn from_path(path: &PathBuf) -> Result<RuleGroup, io::Error> {
    let reader = File::open(path)?;
    let group = serde_yaml::from_reader(reader)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    Ok(group)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGroup {
    pub name: String,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub pattern: RulePattern,
    pub label_name: String,
    pub label_value: String,
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct RulePattern(Regex);

impl RulePattern {
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        Regex::new(pattern).map(RulePattern)
    }
}

impl Serialize for RulePattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.as_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RulePattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        let reg = Regex::new(&raw)
            .map_err(|e| de::Error::custom(format!("unable to parse pattern {:?}: {}", raw, e)))?;
        Ok(RulePattern(reg))
    }
}

impl Deref for RulePattern {
    type Target = Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RulePattern {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
