use crate::planner::{Phase, QueryPlan};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, debug};

/// Tracks the execution state of phases and their dependencies
pub struct PhaseScheduler {
    phases: Vec<Arc<Phase>>,
    phase_dependencies: HashMap<usize, Vec<usize>>, // phase_index -> [dependent_phase_indices]
    completed_queries: Arc<Mutex<HashSet<usize>>>,  // Track completed query IDs
    critical_queries: HashMap<usize, Vec<usize>>,   // phase_index -> [critical_query_ids]
}

#[derive(Debug, Clone)]
pub enum PhaseEvent {
    PhaseStarted { phase_index: usize },
    QueryCompleted { query_id: usize, phase_index: usize },
    PhaseCompleted { phase_index: usize },
    CriticalQueriesCompleted { phase_index: usize },
}

impl PhaseScheduler {
    pub fn new(plan: &QueryPlan) -> Self {
        let phases = plan.phases.iter().map(|p| Arc::new(p.clone())).collect::<Vec<_>>();

        // Analyze dependencies based on ContextSpec
        let phase_dependencies = Self::analyze_dependencies(&phases);
        let critical_queries = Self::identify_critical_queries(&phases);

        info!("Phase scheduler initialized with {} phases", phases.len());
        for (idx, deps) in &phase_dependencies {
            if !deps.is_empty() {
                debug!("Phase {} depends on phases: {:?}", idx, deps);
            }
        }

        Self {
            phases,
            phase_dependencies,
            completed_queries: Arc::new(Mutex::new(HashSet::new())),
            critical_queries,
        }
    }

    /// Analyze phase dependencies based on ContextSpec references
    fn analyze_dependencies(phases: &[Arc<Phase>]) -> HashMap<usize, Vec<usize>> {
        let mut dependencies = HashMap::new();

        // Phase 0 (Overview) has no dependencies
        dependencies.insert(0, vec![]);

        // Phase 1 (Modules) depends on critical queries from Phase 0
        if phases.len() > 1 {
            dependencies.insert(1, vec![0]); // Depends on phase 0's critical queries
        }

        // Phase 2 (API) depends on phases 0 and 1
        if phases.len() > 2 {
            dependencies.insert(2, vec![0, 1]);
        }

        // Phase 3 (Examples) depends on all previous phases
        if phases.len() > 3 {
            dependencies.insert(3, vec![0, 1, 2]);
        }

        dependencies
    }

    /// Identify critical queries that unblock subsequent phases
    fn identify_critical_queries(phases: &[Arc<Phase>]) -> HashMap<usize, Vec<usize>> {
        let mut critical = HashMap::new();

        // For Phase 0, queries 0 and 1 are critical (referenced by later phases)
        if !phases.is_empty() {
            let phase_0_critical: Vec<usize> = phases[0].queries.iter()
                .filter(|q| q.id <= 1) // Queries 0 and 1 are critical
                .map(|q| q.id)
                .collect();
            critical.insert(0, phase_0_critical);
        }

        // Other phases don't have specific critical queries - all must complete
        for i in 1..phases.len() {
            critical.insert(i, vec![]); // Empty means all queries are critical
        }

        critical
    }

    /// Check if a phase can start execution
    pub async fn can_start_phase(&self, phase_index: usize) -> bool {
        let empty_deps = vec![];
        let deps = self.phase_dependencies.get(&phase_index).unwrap_or(&empty_deps);

        if deps.is_empty() {
            return true;
        }

        let completed = self.completed_queries.lock().await;

        for dep_phase_idx in deps {
            let empty_queries = vec![];
            let critical_queries = self.critical_queries.get(dep_phase_idx).unwrap_or(&empty_queries);

            if critical_queries.is_empty() {
                // All queries in the dependency phase must be complete
                let phase_queries: HashSet<usize> = self.phases[*dep_phase_idx]
                    .queries.iter()
                    .map(|q| q.id)
                    .collect();

                if !phase_queries.is_subset(&completed) {
                    return false;
                }
            } else {
                // Only critical queries must be complete
                for query_id in critical_queries {
                    if !completed.contains(query_id) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Mark a query as completed
    pub async fn mark_query_completed(&self, query_id: usize) {
        self.completed_queries.lock().await.insert(query_id);
        debug!("Query {} marked as completed", query_id);
    }

    /// Check if all critical queries for a phase are completed
    pub async fn are_critical_queries_completed(&self, phase_index: usize) -> bool {
        let empty_critical = vec![];
        let critical = self.critical_queries.get(&phase_index).unwrap_or(&empty_critical);

        if critical.is_empty() {
            return true; // No specific critical queries
        }

        let completed = self.completed_queries.lock().await;
        critical.iter().all(|id| completed.contains(id))
    }

    /// Get phases that are ready to execute
    pub async fn get_ready_phases(&self) -> Vec<usize> {
        let mut ready = Vec::new();

        for phase_idx in 0..self.phases.len() {
            if self.can_start_phase(phase_idx).await {
                ready.push(phase_idx);
            }
        }

        ready
    }

    /// Check if a phase is fully completed
    pub async fn is_phase_completed(&self, phase_index: usize) -> bool {
        let phase_queries: HashSet<usize> = self.phases[phase_index]
            .queries.iter()
            .map(|q| q.id)
            .collect();

        let completed = self.completed_queries.lock().await;
        phase_queries.is_subset(&completed)
    }

    /// Get execution strategy summary
    pub fn get_execution_strategy(&self) -> String {
        let mut strategy = String::from("Phase Execution Strategy:\n");

        strategy.push_str("- Phase 0 (Overview): Starts immediately\n");

        if self.phases.len() > 1 {
            strategy.push_str("- Phase 1 (Modules): Starts after Phase 0 critical queries (0, 1) complete\n");
        }

        if self.phases.len() > 2 {
            strategy.push_str("- Phase 2 (API): Starts after Phases 0 and 1 complete\n");
        }

        if self.phases.len() > 3 {
            strategy.push_str("- Phase 3 (Examples): Starts after all previous phases complete\n");
        }

        strategy.push_str("\nThis allows partial parallelization while respecting context dependencies.");

        strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{QueryPriority, ContextSpec, QueryTarget};

    fn create_test_phase(start_id: usize, count: usize) -> Phase {
        let queries = (0..count).map(|i| Query {
            id: start_id + i,
            prompt: format!("Test query {}", start_id + i),
            description: "Test".to_string(),
            context_spec: ContextSpec::None,
            priority: QueryPriority::Medium,
            estimated_tokens: 1000,
            target: QueryTarget::Overview,
        }).collect();

        Phase {
            name: format!("Phase {}", start_id),
            queries,
        }
    }

    #[tokio::test]
    async fn test_phase_dependencies() {
        let plan = QueryPlan {
            phases: vec![
                create_test_phase(0, 3),
                create_test_phase(3, 2),
                create_test_phase(5, 2),
            ],
            total_queries: 7,
            estimated_cost: crate::planner::EstimatedCost {
                input_tokens: 1000,
                output_tokens: 2000,
                total_cost_usd: 0.1,
            },
        };

        let scheduler = PhaseScheduler::new(&plan);

        // Phase 0 should be ready immediately
        assert!(scheduler.can_start_phase(0).await);

        // Phase 1 should not be ready initially
        assert!(!scheduler.can_start_phase(1).await);

        // Mark critical queries as complete
        scheduler.mark_query_completed(0).await;
        scheduler.mark_query_completed(1).await;

        // Now Phase 1 should be ready
        assert!(scheduler.can_start_phase(1).await);
    }
}