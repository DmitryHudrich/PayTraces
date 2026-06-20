use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use domain::chain::ChainId;
use domain::ports::{ChainSource, ChainSourceRegistry};

#[derive(Default, Clone)]
pub struct ChainSources {
    sources: HashMap<ChainId, Arc<dyn ChainSource>>,
}

impl ChainSources {
    pub fn builder() -> ChainSourcesBuilder {
        ChainSourcesBuilder::default()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

#[async_trait]
impl ChainSourceRegistry for ChainSources {
    fn source(&self, chain: ChainId) -> Option<Arc<dyn ChainSource>> {
        self.sources.get(&chain).cloned()
    }

    fn supported_chains(&self) -> Vec<ChainId> {
        let mut v: Vec<_> = self.sources.keys().copied().collect();
        v.sort();
        v
    }
}

#[derive(Default)]
pub struct ChainSourcesBuilder {
    sources: HashMap<ChainId, Arc<dyn ChainSource>>,
}

impl ChainSourcesBuilder {
    pub fn register<S: ChainSource + 'static>(mut self, source: S) -> Self {
        self.sources.insert(source.chain_id(), Arc::new(source));
        self
    }

    pub fn register_arc(mut self, source: Arc<dyn ChainSource>) -> Self {
        self.sources.insert(source.chain_id(), source);
        self
    }

    pub fn build(self) -> ChainSources {
        ChainSources {
            sources: self.sources,
        }
    }
}
