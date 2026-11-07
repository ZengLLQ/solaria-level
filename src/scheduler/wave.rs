use super::graph::ConflictGraph;
use crate::types::Transaction;
use std::collections::HashSet;

/// A wave of transaction indices that have zero write-write or write-read conflicts,
/// and can be safely executed simultaneously across parallel worker threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionWave {
    pub wave_id: usize,
    pub tx_indices: Vec<usize>,
}

/// Wave scheduler generating optimal concurrent execution batches.
pub struct WaveScheduler;

impl WaveScheduler {
    /// Schedule transactions into sequential non-conflicting parallel waves using Kahn's topological sort.
    pub fn schedule(transactions: &[Transaction]) -> Vec<ExecutionWave> {
        let n = transactions.len();
        if n == 0 {
            return Vec::new();
        }

        let graph = ConflictGraph::build(transactions);
        let mut in_degrees: Vec<usize> = (0..n).map(|i| graph.in_degree(i)).collect();
        let mut executed = vec![false; n];
        let mut waves = Vec::new();
        let mut wave_id = 0;

        loop {
            // Find all transactions with in_degree == 0 that have not been executed
            let ready_indices: Vec<usize> = (0..n)
                .filter(|&i| !executed[i] && in_degrees[i] == 0)
                .collect();

            if ready_indices.is_empty() {
                break;
            }

            // Within the ready set, greedily select mutually non-conflicting transactions for this wave
            let mut current_wave_txs = Vec::new();
            let mut wave_writes = HashSet::new();
            let mut wave_reads = HashSet::new();

            for &idx in &ready_indices {
                let tx = &transactions[idx];
                let writes = tx.get_write_set();
                let reads = tx.get_read_set();

                let write_disjoint = wave_writes.is_disjoint(&writes);
                let write_read_disjoint = wave_writes.is_disjoint(&reads) && wave_reads.is_disjoint(&writes);

                if write_disjoint && write_read_disjoint {
                    current_wave_txs.push(idx);
                    wave_writes.extend(writes);
                    wave_reads.extend(reads);
                }
            }

            // Mark selected transactions as executed and decrement dependents' in_degrees
            for &idx in &current_wave_txs {
                executed[idx] = true;
                for &dependent in &graph.outgoing[idx] {
                    if in_degrees[dependent] > 0 {
                        in_degrees[dependent] -= 1;
                    }
                }
            }

            waves.push(ExecutionWave {
                wave_id,
                tx_indices: current_wave_txs,
            });
            wave_id += 1;
        }

        waves
    }

    /// Calculate the theoretical parallel speedup factor: Total Txs / Number of Waves.
    pub fn parallelism_factor(transactions: &[Transaction], waves: &[ExecutionWave]) -> f64 {
        if waves.is_empty() {
            return 1.0;
        }
        transactions.len() as f64 / waves.len() as f64
    }
}
