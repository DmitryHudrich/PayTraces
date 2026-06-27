use std::collections::HashMap;

use domain::primitives::Address;

#[derive(Default)]
pub struct UnionFind {
    parent: HashMap<Address, Address>,
}

impl UnionFind {
    pub fn new() -> Self {
        Self {
            parent: HashMap::new(),
        }
    }

    pub fn insert(&mut self, addr: Address) {
        self.parent.entry(addr.clone()).or_insert(addr);
    }

    pub fn find(&mut self, addr: &Address) -> Address {
        self.insert(addr.clone());
        let parent = self.parent.get(addr).cloned().unwrap();
        if &parent == addr {
            return parent;
        }
        let root = self.find(&parent);
        self.parent.insert(addr.clone(), root.clone());
        root
    }

    pub fn union(&mut self, a: &Address, b: &Address) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }

    pub fn union_all(&mut self, addrs: &[Address]) {
        if addrs.len() < 2 {
            for a in addrs {
                self.insert(a.clone());
            }
            return;
        }
        let first = addrs[0].clone();
        for a in addrs.iter().skip(1) {
            self.union(&first, a);
        }
    }

    pub fn components(&mut self) -> Vec<Vec<Address>> {
        let keys: Vec<Address> = self.parent.keys().cloned().collect();
        let mut groups: HashMap<Address, Vec<Address>> = HashMap::new();
        for k in keys {
            let root = self.find(&k);
            groups.entry(root).or_default().push(k);
        }
        for v in groups.values_mut() {
            v.sort_by(|a, b| a.bytes().cmp(b.bytes()));
        }
        groups.into_values().collect()
    }
}
