use fst::raw;

mod boilerplate;
pub use self::boilerplate::PrefixSet;
pub use self::boilerplate::PrefixSetBuilder;

#[cfg(test)] mod tests;

impl PrefixSet {
    pub fn contains_prefix<B: AsRef<[u8]>>(&self, key: B) -> bool {
        let fst = &self.as_fst();
        let mut node = fst.root();
        for &b in key.as_ref() {
            node = match node.find_input(b) {
                None => return false,
                Some(i) => fst.node(node.transition_addr(i)),
            }
        }
        true
    }

    pub fn get_prefix_range<B: AsRef<[u8]>>(&self, key: B) -> Option<(raw::Output, raw::Output)> {
        let fst = &self.as_fst();
        let mut node = fst.root();
        let mut out = raw::Output::zero();
        for &b in key.as_ref() {
            node = match node.find_input(b) {
                None => return None,
                Some(i) => {
                    let t = node.transition(i);
                    out = out.cat(t.out);
                    fst.node(t.addr)
                }
            }
        }
        let start = out.cat(node.final_output());

        while node.len() != 0 {
            let t = node.transition(node.len() - 1);
            out = out.cat(t.out);
            node = fst.node(t.addr);
        }
        Some((start, out.cat(node.final_output()).cat(raw::Output::new(1))))
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