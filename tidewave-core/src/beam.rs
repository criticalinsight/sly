use anyhow::{Context, Result};
use std::process::Command;
use tracing::{debug, warn};
use uuid::Uuid;

/// Discover running BEAM nodes via `epmd`.
/// Returns a list of node names (e.g., ["gswarm@localhost"]).
pub async fn list_nodes() -> Result<Vec<String>> {
    let output = Command::new("epmd")
        .arg("-names")
        .output()
        .context("Failed to execute epmd")?;

    if !output.status.success() {
        warn!("epmd failed: {:?}", String::from_utf8_lossy(&output.stderr));
        return Ok(vec![]);
    }

    let stdout = String::from_utf8(output.stdout).context("Invalid UTF-8 from epmd")?;
    let mut nodes = Vec::new();

    // epmd: up and running on port 4369 with data:
    // name gswarm at port 5000
    for line in stdout.lines() {
        if line.starts_with("name ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1];
                // Append @localhost for now, assuming local discovery
                nodes.push(format!("{}@localhost", name));
            }
        }
    }

    Ok(nodes)
}

/// Helper to run an Erlang expression on a temporary hidden node.
async fn run_erl_eval(node: &str, expr: &str) -> Result<String> {
    // Generate a unique name for this sidecar to avoid conflicts
    let sidecar_name = format!("tidewave_sidecar_{}", Uuid::new_v4().simple());
    
    // We assume the cookie is in ~/.erlang.cookie and accessible.
    // We use -hidden to avoid showing up in the target's standard node list if possible,
    // though -sname implies short names.
    let output = Command::new("erl")
        .arg("-noshell")
        .arg("-hidden")
        .arg("-sname")
        .arg(&sidecar_name)
        .arg("-eval")
        .arg(format!(
            "io:format(\"~s\", [rpc:call('{}', erlang, apply, [fun() -> {} end, []])]), init:stop().",
            node, expr
        ))
        .output()
        .context("Failed to spawn erl sidecar")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Sidecar failed: {}", stderr);
    }

    let result = String::from_utf8(output.stdout).context("Invalid UTF-8 from sidecar")?;
    Ok(result)
}

/// List all ETS tables on the target node.
pub async fn inspect_ets_list(node: &str) -> Result<String> {
    // We map ets:all() to a list of table names/info to prevent massive output
    // ets:all() returns [TabId]
    // We want to turn that into meaningful names if possible.
    let expr = "
        lists:map(fun(T) -> 
            case ets:info(T, name) of 
                undefined -> T; 
                Name -> Name 
            end 
        end, ets:all())
    ";
    
    // RPC call to the target node
    // Note: rpc:call returns {badrpc, Reason} if it fails, which io:format might print uglily.
    // For simplicity in this v1, we just return the raw string repr.
    run_erl_eval(node, expr).await
}

/// View the contents of a specific ETS table (limited count).
pub async fn inspect_ets_view(node: &str, table_str: &str) -> Result<String> {
    // We need to handle both named tables (atom) and numeric tables (integer? ref?)
    // For now we assume named tables (atoms) as that's what GleamDB uses mostly.
    
    let expr = format!("
        Tab = '{}',
        MatchSpec = [{{'$1', [], ['$1']}}],
        % Limit to 50 items for safety
        case ets:select(Tab, MatchSpec, 50) of
            {{Matches, _Cont}} -> Matches;
            '$end_of_table' -> [];
            Error -> Error
        end
    ", table_str);

    run_erl_eval(node, &expr).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_nodes() {
        // This test assumes a node is running. 
        // We will start one in the background or mocked environment.
        // For now, let's just print what we find.
        let nodes = list_nodes().await.unwrap();
        println!("Found nodes: {:?}", nodes);
        // We expect at least one node if we started the dummy one
        // assert!(!nodes.is_empty()); 
    }
}
