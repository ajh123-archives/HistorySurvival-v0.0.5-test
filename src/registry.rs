use std::collections::HashMap;

#[derive(Debug)]
pub enum RegistryError {
    KeyAlreadyExists { key: String, },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::KeyAlreadyExists { key } => write!(f, "key already exists in the Registry: {}", key),
        }
    }
}

impl std::error::Error for RegistryError {}

/// A way to sort elements by name
pub struct Registry<T> {
    name_to_id: HashMap<String, u32>,
    id_to_name: Vec<String>,
    id_to_value: Vec<T>,
}

impl<T> Registry<T> {
    pub fn register(&mut self, name: String, value: T) -> Result<u32, RegistryError> {
        if self.name_to_id.contains_key(&name) {
            Err(RegistryError::KeyAlreadyExists { key: name })
        } else {
            let id = self.id_to_name.len() as u32;
            self.id_to_name.push(name.clone());
            self.name_to_id.insert(name, id);
            self.id_to_value.push(value);
            Ok(id)
        }
    }

    pub fn get_id_by_name(&self, name: &String) -> Option<u32> {
        self.name_to_id.get(name).cloned()
    }
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            name_to_id: HashMap::new(),
            id_to_name: Vec::new(),
            id_to_value: Vec::new(),
        }
    }
}