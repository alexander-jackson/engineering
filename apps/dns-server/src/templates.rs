use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct IndexContext {
    pub domains: Vec<String>,
}

impl IndexContext {
    pub fn new(domains: impl IntoIterator<Item = String>) -> Self {
        let mut domains: Vec<_> = domains.into_iter().collect();
        domains.sort();

        Self { domains }
    }
}
