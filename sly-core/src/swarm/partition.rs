//! Dependency-Aware Partitioning
//!
//! Phase 3: Smart task partitioning based on dependency graph.
//! Groups files by strongly connected components for parallel execution.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use crate::swarm::context::SwarmContext;
use crate::swarm::task::SwarmTask;

/// Partitioning strategy
#[derive(Debug, Clone, Copy)]
pub enum PartitionStrategy {
    /// One file per task (default, for independent operations)
    PerFile,
    /// Group by module/directory
    PerModule,
    /// Group by strongly connected component (for type changes)
    BySCC,
    /// Single task for everything
    Single,
}

/// Smart partitioner for swarm tasks
pub struct TaskPartitioner {
    strategy: PartitionStrategy,
}

impl TaskPartitioner {
    pub fn new(strategy: PartitionStrategy) -> Self {
        Self { strategy }
    }

    /// Auto-detect best strategy based on task
    pub fn auto(instruction: &str) -> Self {
        let strategy = if instruction.contains("format") || instruction.contains("lint") {
            PartitionStrategy::PerFile
        } else if instruction.contains("type") || instruction.contains("interface") {
            PartitionStrategy::BySCC
        } else if instruction.contains("refactor") {
            PartitionStrategy::PerModule
        } else {
            PartitionStrategy::PerFile
        };
        
        Self { strategy }
    }

    /// Partition files into tasks based on strategy
    pub fn partition(
        &self,
        instruction: &str,
        files: Vec<PathBuf>,
        context: Option<&SwarmContext>,
    ) -> Vec<SwarmTask> {
        match self.strategy {
            PartitionStrategy::PerFile => self.partition_per_file(instruction, files),
            PartitionStrategy::PerModule => self.partition_per_module(instruction, files),
            PartitionStrategy::BySCC => self.partition_by_scc(instruction, files, context),
            PartitionStrategy::Single => self.partition_single(instruction, files),
        }
    }

    fn partition_per_file(&self, instruction: &str, files: Vec<PathBuf>) -> Vec<SwarmTask> {
        files
            .into_iter()
            .enumerate()
            .map(|(i, file)| {
                SwarmTask::new(
                    &format!("file-{}", i),
                    &format!("{}\n\nTarget: {}", instruction, file.display()),
                )
                .with_files(vec![file.to_string_lossy().to_string()])
            })
            .collect()
    }

    fn partition_per_module(&self, instruction: &str, files: Vec<PathBuf>) -> Vec<SwarmTask> {
        // Group by parent directory
        let mut modules: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        
        for file in files {
            let module = file.parent().unwrap_or(&file).to_path_buf();
            modules.entry(module).or_default().push(file);
        }
        
        modules
            .into_iter()
            .enumerate()
            .map(|(i, (module, files))| {
                let file_list = files
                    .iter()
                    .map(|f| f.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                
                SwarmTask::new(
                    &format!("module-{}", i),
                    &format!(
                        "{}\n\nModule: {}\nFiles: {}",
                        instruction,
                        module.display(),
                        file_list
                    ),
                )
                .with_files(files.iter().map(|f| f.to_string_lossy().to_string()).collect())
            })
            .collect()
    }

    fn partition_by_scc(
        &self,
        instruction: &str,
        files: Vec<PathBuf>,
        context: Option<&SwarmContext>,
    ) -> Vec<SwarmTask> {
        let context = match context {
            Some(c) => c,
            None => return self.partition_per_file(instruction, files),
        };

        // Build adjacency list for SCC computation
        let file_set: HashSet<PathBuf> = files.iter().cloned().collect();
        let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        
        for file in &files {
            let deps = context.dependencies_of(file);
            let filtered_deps: Vec<_> = deps
                .into_iter()
                .filter(|d| file_set.contains(d))
                .collect();
            
            let dependents = context.dependents(file);
            let filtered_dependents: Vec<_> = dependents
                .into_iter()
                .filter(|d| file_set.contains(d))
                .collect();
            
            // Bidirectional edges for SCC
            for dep in filtered_deps {
                graph.entry(file.clone()).or_default().push(dep);
            }
            for dep in filtered_dependents {
                graph.entry(dep).or_default().push(file.clone());
            }
        }


        // Simple connected components (not true SCC, but good enough)
        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut components: Vec<Vec<PathBuf>> = Vec::new();
        
        for file in &files {
            if visited.contains(file) {
                continue;
            }
            
            let mut component = Vec::new();
            let mut stack = vec![file.clone()];
            
            while let Some(current) = stack.pop() {
                if visited.contains(&current) {
                    continue;
                }
                visited.insert(current.clone());
                component.push(current.clone());
                
                if let Some(neighbors) = graph.get(&current) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            stack.push(neighbor.clone());
                        }
                    }
                }
            }
            
            if !component.is_empty() {
                components.push(component);
            }
        }


        components
            .into_iter()
            .enumerate()
            .map(|(i, files)| {
                let file_list = files
                    .iter()
                    .map(|f| f.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                
                SwarmTask::new(
                    &format!("scc-{}", i),
                    &format!(
                        "{}\n\nConnected component {} ({} files): {}",
                        instruction,
                        i,
                        files.len(),
                        file_list
                    ),
                )
                .with_files(files.iter().map(|f| f.to_string_lossy().to_string()).collect())
            })
            .collect()
    }

    fn partition_single(&self, instruction: &str, files: Vec<PathBuf>) -> Vec<SwarmTask> {
        vec![SwarmTask::new("single", instruction)
            .with_files(files.iter().map(|f| f.to_string_lossy().to_string()).collect())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_per_file_partition() {
        let partitioner = TaskPartitioner::new(PartitionStrategy::PerFile);
        let files = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        let tasks = partitioner.partition("refactor", files, None);
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_per_module_partition() {
        let partitioner = TaskPartitioner::new(PartitionStrategy::PerModule);
        let files = vec![
            PathBuf::from("src/foo/a.rs"),
            PathBuf::from("src/foo/b.rs"),
            PathBuf::from("src/bar/c.rs"),
        ];
        let tasks = partitioner.partition("refactor", files, None);
        assert_eq!(tasks.len(), 2); // Two modules
    }

    #[test]
    fn test_auto_detect_format() {
        let partitioner = TaskPartitioner::auto("format all files");
        assert!(matches!(partitioner.strategy, PartitionStrategy::PerFile));
    }

    #[test]
    fn test_auto_detect_type() {
        let partitioner = TaskPartitioner::auto("change interface type");
        assert!(matches!(partitioner.strategy, PartitionStrategy::BySCC));
    }
}
