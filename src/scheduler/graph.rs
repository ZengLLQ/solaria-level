use crate::types::Transaction;
use std::collections::{HashMap, HashSet};

/// Directed dependency graph modeling RAW, WAR, and WAW conflicts between transactions.
#[derive(Debug, Clone)]
pub struct ConflictGraph {
    pub num_txs: usize,
    /// Adjacency list: tx_i -> set of tx_j that depend on tx_i (i < j)
    pub outgoing: Vec<HashSet<usize>>,
    /// Predecessor list: tx_j -> set of tx_i that tx_j depends on (i < j)
    pub incoming: Vec<HashSet<usize>>,
}

impl ConflictGraph {
    /// Construct dependency graph from an ordered list of transactions.
    pub fn build(transactions: &[Transaction]) -> Self {
        let n = transactions.len();
        let mut outgoing = vec![HashSet::new(); n];
        let mut incoming = vec![HashSet::new(); n];

        // Track last writer and current readers for each account
        let mut last_writer: HashMap<crate::types::Pubkey, usize> = HashMap::new();
        let mut current_readers: HashMap<crate::types::Pubkey, Vec<usize>> = HashMap::new();

        for (curr_idx, tx) in transactions.iter().enumerate() {
            let writes = tx.get_write_set();
            let reads = tx.get_read_set();

            // 1. Dependency on previous writes (RAW and WAW)
            for acc in writes.iter().chain(reads.iter()) {
                if let Some(&writer_idx) = last_writer.get(acc) {
                    if writer_idx < curr_idx {
                        outgoing[writer_idx].insert(curr_idx);
                        incoming[curr_idx].insert(writer_idx);
                    }
                }
            }

            // 2. Dependency on previous reads if current tx writes (WAR)
            for acc in &writes {
                if let Some(readers) = current_readers.get(acc) {
                    for &reader_idx in readers {
                        if reader_idx < curr_idx {
                            outgoing[reader_idx].insert(curr_idx);
                            incoming[curr_idx].insert(reader_idx);
                        }
                    }
                }
            }

            // 3. Update account tracking
            for acc in writes {
                last_writer.insert(acc, curr_idx);
                // Writing clobbers previous read set for subsequent transactions
                current_readers.remove(&acc);
            }
            for acc in reads {
                current_readers.entry(acc).or_default().push(curr_idx);
            }
        }

        Self {
            num_txs: n,
            outgoing,
            incoming,
        }
    }

    /// Number of dependencies transaction tx_idx is waiting on.
    #[inline]
    pub fn in_degree(&self, tx_idx: usize) -> usize {
        self.incoming[tx_idx].len()
    }
}
