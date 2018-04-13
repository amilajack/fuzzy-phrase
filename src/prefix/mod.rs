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
        Some((start, out.cat(node.final_output())))
    }
}