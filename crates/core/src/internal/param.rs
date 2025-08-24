use crate::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSet {
    pub values: IndexMap<String, Value>,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl ParameterSet {
    pub fn new() -> Self {
        Self {
            values: IndexMap::new(),
            name: None,
            description: None,
        }
    }

    pub fn with_param<K, V>(mut self, key: K, value: V) -> Result<Self>
    where
        K: Into<String>,
        V: Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        self.values.insert(key.into(), json_value);
        Ok(self)
    }

    pub fn with_name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn get<T>(&self, key: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.values
            .get(key)
            .ok_or_else(|| Error::test_setup(format!("Parameter '{}' not found", key)))
            .and_then(|v| serde_json::from_value(v.clone()).map_err(Error::from))
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    pub fn display_name(&self) -> String {
        if let Some(ref name) = self.name {
            name.clone()
        } else {
            let param_strings: Vec<String> = self
                .values
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            format!("[{}]", param_strings.join(", "))
        }
    }
}

impl Default for ParameterSet {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterCollection {
    pub sets: Vec<ParameterSet>,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl ParameterCollection {
    pub fn new() -> Self {
        Self {
            sets: Vec::new(),
            name: None,
            description: None,
        }
    }

    pub fn add_set(mut self, set: ParameterSet) -> Self {
        self.sets.push(set);
        self
    }

    pub fn with_name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Get the cartesian product of the sets in this collection.
    ///
    /// TODO: use `itertools` for this
    pub fn cartesian_product(parameters: IndexMap<String, Vec<Value>>) -> Self {
        let mut sets = Vec::new();

        if parameters.is_empty() {
            return Self::new();
        }

        let keys: Vec<String> = parameters.keys().cloned().collect();
        let values: Vec<Vec<Value>> = parameters.values().cloned().collect();

        fn generate_combinations(
            keys: &[String],
            values: &[Vec<Value>],
            current: &mut IndexMap<String, Value>,
            index: usize,
            results: &mut Vec<ParameterSet>,
        ) {
            if index == keys.len() {
                let mut set = ParameterSet::new();
                set.values = current.clone();
                results.push(set);
                return;
            }

            for value in &values[index] {
                current.insert(keys[index].clone(), value.clone());
                generate_combinations(keys, values, current, index + 1, results);
                current.swap_remove(&keys[index]);
            }
        }

        let mut current = IndexMap::new();
        generate_combinations(&keys, &values, &mut current, 0, &mut sets);

        Self {
            sets,
            name: None,
            description: None,
        }
    }

    pub fn from_objects<T>(objects: Vec<T>) -> Result<Self>
    where
        T: Serialize,
    {
        let mut sets = Vec::new();

        for (index, obj) in objects.into_iter().enumerate() {
            let value = serde_json::to_value(obj)?;

            if let Value::Object(map) = value {
                let mut param_set = ParameterSet::new();
                for (key, val) in map {
                    param_set.values.insert(key, val);
                }
                param_set.name = Some(format!("Set {}", index + 1));
                sets.push(param_set);
            } else {
                return Err(Error::test_setup("Object must serialize to JSON object"));
            }
        }

        Ok(Self {
            sets,
            name: None,
            description: None,
        })
    }

    #[cfg(feature = "csv")]
    pub fn from_csv(csv_data: &str, has_headers: bool) -> Result<Self> {
        let mut reader = csv::Reader::from_reader(csv_data.as_bytes());
        let mut sets = Vec::new();

        if has_headers {
            let headers = reader
                .headers()
                .map_err(|e| Error::test_setup(format!("CSV parse error: {}", e)))?
                .clone();

            for (index, result) in reader.records().enumerate() {
                let record =
                    result.map_err(|e| Error::test_setup(format!("CSV parse error: {}", e)))?;

                let mut param_set = ParameterSet::new();
                for (i, field) in record.iter().enumerate() {
                    if let Some(header) = headers.get(i) {
                        let value = serde_json::from_str(field)
                            .unwrap_or_else(|_| Value::String(field.to_string()));
                        param_set.values.insert(header.to_string(), value);
                    }
                }
                param_set.name = Some(format!("Row {}", index + 1));
                sets.push(param_set);
            }
        } else {
            for (index, result) in reader.records().enumerate() {
                let record =
                    result.map_err(|e| Error::test_setup(format!("CSV parse error: {}", e)))?;

                let mut param_set = ParameterSet::new();
                for (i, field) in record.iter().enumerate() {
                    let value = serde_json::from_str(field)
                        .unwrap_or_else(|_| Value::String(field.to_string()));
                    param_set.values.insert(format!("col_{}", i), value);
                }
                param_set.name = Some(format!("Row {}", index + 1));
                sets.push(param_set);
            }
        }

        Ok(Self {
            sets,
            name: None,
            description: Some("Generated from CSV data".to_string()),
        })
    }

    pub fn len(&self) -> usize {
        self.sets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ParameterSet> {
        self.sets.iter()
    }
}

impl Default for ParameterCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for ParameterCollection {
    type Item = ParameterSet;
    type IntoIter = std::vec::IntoIter<ParameterSet>;

    fn into_iter(self) -> Self::IntoIter {
        self.sets.into_iter()
    }
}

pub struct ParameterBuilder {
    parameters: IndexMap<String, Vec<Value>>,
}

impl ParameterBuilder {
    pub fn new() -> Self {
        Self {
            parameters: IndexMap::new(),
        }
    }

    pub fn add_param<K, V>(mut self, key: K, values: Vec<V>) -> Result<Self>
    where
        K: Into<String>,
        V: Serialize,
    {
        let json_values: Result<Vec<Value>> = values
            .into_iter()
            .map(|v| serde_json::to_value(v).map_err(Error::from))
            .collect();

        self.parameters.insert(key.into(), json_values?);
        Ok(self)
    }

    pub fn build(self) -> ParameterCollection {
        ParameterCollection::cartesian_product(self.parameters)
    }
}

impl Default for ParameterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
