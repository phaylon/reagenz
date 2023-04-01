use std::collections::HashMap;

use derivative::Derivative;
use smol_str::SmolStr;


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Index(usize);

#[derive(Derivative, Clone)]
#[derivative(Default(bound=""))]
pub struct IdMap<N, D> {
    indices: HashMap<SmolStr, Index>,
    nodes: Vec<N>,
    data: Vec<D>,
}

impl<N, D> IdMap<N, D> {
    pub fn set(&mut self, id: SmolStr, node: N, data: D) -> Index {
        if let Some(&index) = self.indices.get(&id) {
            self.nodes[index.0] = node;
            self.data[index.0] = data;
            index
        } else {
            let index = Index(self.nodes.len());
            self.indices.insert(id, index);
            self.nodes.push(node);
            self.data.push(data);
            index
        }
    }

    pub fn indices(&self) -> impl Iterator<Item = Index> {
        (0..self.nodes.len()).into_iter().map(Index)
    }

    pub fn find(&self, id: &str) -> Option<Index> {
        self.indices.get(id).copied()
    }

    pub fn name(&self, index: Index) -> Option<&SmolStr> {
        for (name, name_index) in &self.indices {
            if index == *name_index {
                return Some(name);
            }
        }
        None
    }

    #[track_caller]
    pub fn set_node(&mut self, index: Index, node: N) {
        *self.nodes.get_mut(index.0).expect("id index is invalid") = node;
    }

    #[track_caller]
    pub fn node(&self, index: Index) -> &N {
        self.nodes.get(index.0).expect("id index is invalid")
    }

    #[track_caller]
    pub fn data(&self, index: Index) -> &D {
        self.data.get(index.0).expect("id index is invalid")
    }
}
