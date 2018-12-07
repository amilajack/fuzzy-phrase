use fst::raw;

mod boilerplate;
pub use self::boilerplate::PrefixSet;
pub use self::boilerplate::PrefixSetBuilder;

#[cfg(test)] mod tests;

impl PrefixSet {
    pub fn lookup<B: AsRef<[u8]>>(&self, key: B) -> PrefixSetLookupResult {
        let fst = &self.as_fst();
        let mut node = fst.root();
        let mut out = raw::Output::zero();
        for &b in key.as_ref() {
            node = match node.find_input(b) {
                None => return PrefixSetLookupResult::NotFound,
                Some(i) => {
                    let t = node.transition(i);
                    out = out.cat(t.out);
                    fst.node(t.addr)
                }
            }
        }
        PrefixSetLookupResult::Found { fst, node, output_so_far: out }
    }

    pub fn get_by_id(&self, id: raw::Output) -> Option<Vec<u8>> {
        let mut id = id.clone();
        let fst = &self.as_fst();
        let mut node = fst.root();
        let mut key: Vec<u8> = Vec::new();

        loop {
            let mut next_node: Option<_> = None;
            {
                let mut transitions = node.transitions().peekable();
                while let Some(current) = transitions.next() {
                    let found = match transitions.peek() {
                        Some(next) => next.out > id,
                        None => true,
                    };
                    if found {
                        if current.out > id {
                            return None;
                        }

                        id = id.sub(current.out);
                        key.push(current.inp);

                        let nn = fst.node(current.addr);
                        if id.value() == 0 && nn.is_final() {
                            return Some(key);
                        } else {
                            next_node = Some(nn);
                        }
                        break;
                    }
                }
            }

            match next_node {
                Some(n) => node = n,
                None => return None,
            }
        }
    }
}

pub enum PrefixSetLookupResult<'a> {
    NotFound,
    Found { fst: &'a raw::Fst, node: raw::Node<'a>, output_so_far: raw::Output }
}

impl<'a> PrefixSetLookupResult<'a> {
    pub fn found(&self) -> bool {
        match *self {
            PrefixSetLookupResult::NotFound => false,
            PrefixSetLookupResult::Found {..} => true
        }
    }

    pub fn found_final(&self) -> bool {
        match *self {
            PrefixSetLookupResult::NotFound => false,
            PrefixSetLookupResult::Found { node, .. } => node.is_final()
        }
    }

    pub fn id(&self) -> Option<raw::Output> {
        match *self {
            PrefixSetLookupResult::NotFound => None,
            PrefixSetLookupResult::Found { node, output_so_far, .. } => {
                if node.is_final() {
                    Some(output_so_far.cat(node.final_output()))
                } else {
                    None
                }
            }
        }
    }

    pub fn range(&self) -> Option<(raw::Output, raw::Output)> {
        match *self {
            PrefixSetLookupResult::NotFound => None,
            PrefixSetLookupResult::Found { fst, node, output_so_far } => {
                let mut node: raw::Node = node.to_owned();
                let mut out: raw::Output = output_so_far.to_owned();
                let start = out.cat(node.final_output());

                while node.len() != 0 {
                    let t = node.transition(node.len() - 1);
                    out = out.cat(t.out);
                    node = fst.node(t.addr);
                }
                Some((start, out.cat(node.final_output())))
            }
        }
    }

    pub fn has_continuations(&self) -> bool {
        match *self {
            PrefixSetLookupResult::NotFound => false,
            PrefixSetLookupResult::Found { node, .. } => node.len() > 0
        }
    }
}