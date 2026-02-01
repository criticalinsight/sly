# Sly as an MCP Server: Design Document

> **"It is better to have 100 functions operate on one data structure than 10 functions on 10 structures."** — Alan Perlis (via Rich Hickey)

## Executive Summary

Sly can function as an **MCP (Model Context Protocol) Server** by exposing its internal capabilities—autonomous coding, knowledge retrieval, memory persistence, and safety-hardened execution—as standardized MCP tools. This would allow **any MCP client** (Claude Desktop, Cursor, Zed, etc.) to leverage Sly's Rust-native intelligence without requiring direct integration.

This document outlines the architectural approach, tool surface area, and implementation strategy for transforming Sly from an **MCP client** (consuming external tools) into an **MCP server** (providing tools to external agents).

---

## Current State: Sly as MCP Client

Sly currently operates as an **MCP client** with the following capabilities:

### 1. **Client Architecture** (`src/mcp/client.rs`)

- JSON-RPC 2.0 transport over stdio
- Handshake protocol: `initialize` → `notifications/initialized`
- Tool discovery: `tools/list`
- Tool execution: `tools/call`

### 2. **Auto-Discovery** (`src/mcp/discovery.rs`)

- Scans `~/.sly/mcp/` for executable MCP servers
- Spawns child processes and establishes stdio communication
- Registers discovered servers in a global `HashMap<String, Arc<McpClient>>`

### 3. **Local Native Tools** (`src/mcp/local.rs`)

Sly has a trait-based system for **internal tools**:

```rust
#[async_trait]
pub trait LocalMcp: Send + Sync {
    fn name(&self) -> &str;
    fn tool_definitions(&self) -> String;
    async fn execute(&self, tool_name: &str, arguments: &Value) -> Result<Value>;
}
```

Current implementations:

- **BrowserMcp**: Headless Chrome automation
- **CloudMcp**: Cloudflare/AWS deployment
- **FetchMcp**: HTTP requests

### 4. **Universal Knowledge Retrieval (UKR)**

A meta-tool that broadcasts search queries across all connected MCP servers:

```rust
if tool_name == "ukr_search" {
    // Broadcast to all servers with search capabilities
    for meta in metadata {
        if meta.name.contains("search") {
            results.push(meta.client.call_tool(&meta.name, query).await);
        }
    }
}
```

---

## Vision: Sly as MCP Server

### Core Concept: **Inversion of Control**

Instead of Sly **consuming** MCP tools, Sly **exposes** its capabilities as MCP tools. This enables:

1. **Claude Desktop** to invoke `sly_autonomous_fix` when encountering a build error
2. **Cursor** to call `sly_knowledge_search` for codebase-specific context
3. **Zed** to use `sly_memory_recall` for cross-session heuristic retrieval
4. **Any MCP Client** to leverage Sly's safety-hardened execution (OverlayFS, Reflexion)

### Architectural Alignment with Rich Hickey's Philosophy

This transformation embodies **decomplection**:

- **Separation of Mechanism from Policy**: The MCP server is a pure transport mechanism. The policy (what tools to expose, how to execute them) remains in Sly's core.
- **Data-Oriented Design**: Tools are described as **data** (JSON schemas), not code interfaces.
- **Composability**: External agents can compose Sly's tools with their own capabilities without tight coupling.

---

## Proposed MCP Tool Surface

### Category 1: **Autonomous Execution**

#### `sly_autonomous_task`

**Description**: Execute a complete autonomous coding task with safety guarantees.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "task": {
      "type": "string",
      "description": "Natural language task description"
    },
    "max_loops": {
      "type": "integer",
      "default": 50,
      "description": "Circuit breaker for API spend"
    },
    "thinking_level": {
      "type": "string",
      "enum": ["minimal", "low", "high"],
      "default": "low"
    },
    "use_overlay": {
      "type": "boolean",
      "default": true,
      "description": "Enable OverlayFS safety shield"
    }
  },
  "required": ["task"]
}
```

**Output**: Structured result with:

- Files modified
- Tests run
- Verification status
- Commit-ready overlay snapshot

**Implementation**: Delegates to `cortex_loop` with ephemeral memory mode.

---

#### `sly_reflexion_fix`

**Description**: Self-healing error correction loop.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "error_output": {
      "type": "string",
      "description": "stderr or error message to analyze"
    },
    "context_files": {
      "type": "array",
      "items": {"type": "string"},
      "description": "Relevant file paths for context"
    }
  },
  "required": ["error_output"]
}
```

**Output**: Fixed code + verification proof.

**Implementation**: Wraps `src/core/reflexion.rs` logic.

---

### Category 2: **Knowledge & Memory**

#### `sly_knowledge_search`

**Description**: Semantic search across codebase with graph-guided context.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Search query"
    },
    "max_results": {
      "type": "integer",
      "default": 10
    },
    "include_neighbors": {
      "type": "boolean",
      "default": true,
      "description": "Include graph neighbors in results"
    }
  },
  "required": ["query"]
}
```

**Output**: Ranked results with file paths, line ranges, and relevance scores.

**Implementation**: Uses `src/knowledge/scanner.rs` + CozoDB graph queries.

---

#### `sly_memory_recall`

**Description**: Retrieve cross-session heuristics and learned patterns.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "context": {
      "type": "string",
      "description": "Context for heuristic retrieval (e.g., 'rust error handling')"
    },
    "limit": {
      "type": "integer",
      "default": 5
    }
  },
  "required": ["context"]
}
```

**Output**: List of relevant heuristics with confidence scores.

**Implementation**: Queries CozoDB `heuristics` relation.

---

#### `sly_memory_store`

**Description**: Persist a new heuristic for future sessions.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "pattern": {
      "type": "string",
      "description": "The pattern or rule to remember"
    },
    "context": {
      "type": "string",
      "description": "Context tags (e.g., 'rust,async,tokio')"
    },
    "confidence": {
      "type": "number",
      "default": 0.8
    }
  },
  "required": ["pattern", "context"]
}
```

**Output**: Confirmation with heuristic ID.

---

### Category 3: **Safety & Verification**

#### `sly_overlay_execute`

**Description**: Execute a speculative file operation in OverlayFS.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "operations": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "type": {"type": "string", "enum": ["write", "delete"]},
          "path": {"type": "string"},
          "content": {"type": "string"}
        }
      }
    },
    "auto_commit": {
      "type": "boolean",
      "default": false
    }
  },
  "required": ["operations"]
}
```

**Output**: Overlay snapshot ID + diff preview.

**Implementation**: Wraps `src/safety/overlay.rs`.

---

#### `sly_verify_changes`

**Description**: Run verification suite on overlay snapshot.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "snapshot_id": {
      "type": "string",
      "description": "OverlayFS snapshot to verify"
    },
    "tests": {
      "type": "array",
      "items": {"type": "string"},
      "description": "Specific test patterns to run"
    }
  },
  "required": ["snapshot_id"]
}
```

**Output**: Test results + safety audit.

---

### Category 4: **Workflow Orchestration**

#### `sly_workflow_execute`

**Description**: Run a discovered workflow from `.agent/workflows/`.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "workflow_name": {
      "type": "string",
      "description": "Workflow file name (without .md)"
    },
    "variables": {
      "type": "object",
      "description": "Template variables for workflow"
    }
  },
  "required": ["workflow_name"]
}
```

**Output**: Workflow execution log.

**Implementation**: Delegates to `src/core/supervisor.rs` workflow discovery.

---

### Category 5: **Meta Tools**

#### `sly_ukr_search`

**Description**: Broadcast search across Sly's connected MCP servers.

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "query": {"type": "string"}
  },
  "required": ["query"]
}
```

**Output**: Aggregated results from all MCP servers.

**Implementation**: Reuses existing UKR logic from `src/mcp/registry.rs`.

---

## Implementation Strategy

### Phase 1: **Server Transport Layer**

Create `src/mcp/server.rs`:

```rust
pub struct McpServer {
    tools: Vec<Box<dyn LocalMcp>>,
    transport: Box<dyn ServerTransport>,
}

impl McpServer {
    pub async fn serve(&self) -> Result<()> {
        loop {
            let request = self.transport.receive_request().await?;
            
            match request.method.as_str() {
                "initialize" => self.handle_initialize(request).await?,
                "tools/list" => self.handle_list_tools(request).await?,
                "tools/call" => self.handle_call_tool(request).await?,
                _ => self.send_error(request.id, "Method not found").await?,
            }
        }
    }
}
```

### Phase 2: **Tool Registration**

Extend `LocalMcp` trait to support MCP schema generation:

```rust
#[async_trait]
pub trait LocalMcp: Send + Sync {
    fn name(&self) -> &str;
    fn tool_definitions(&self) -> String; // XML format (legacy)
    fn mcp_tools(&self) -> Vec<Tool>; // NEW: JSON Schema format
    async fn execute(&self, tool_name: &str, arguments: &Value) -> Result<Value>;
}
```

Implement for each tool category:

```rust
pub struct AutonomousMcp;

impl LocalMcp for AutonomousMcp {
    fn mcp_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "sly_autonomous_task".to_string(),
                description: Some("Execute autonomous coding task".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task": {"type": "string"},
                        "max_loops": {"type": "integer", "default": 50}
                    },
                    "required": ["task"]
                }),
            }
        ]
    }
    
    async fn execute(&self, tool_name: &str, args: &Value) -> Result<Value> {
        match tool_name {
            "sly_autonomous_task" => {
                let task = args["task"].as_str().unwrap();
                // Delegate to cortex_loop
                Ok(json!({"status": "completed"}))
            }
            _ => Err(SlyError::Mcp("Unknown tool".to_string()))
        }
    }
}
```

### Phase 3: **Dual-Mode Operation**

Sly should support **both** client and server modes:

```rust
#[derive(Parser)]
enum SlyMode {
    /// Run as autonomous agent (default)
    Agent,
    
    /// Run as MCP server (stdio mode)
    McpServer,
    
    /// Initialize workspace
    Init,
}
```

In `main.rs`:

```rust
match mode {
    SlyMode::Agent => {
        // Existing cortex_loop logic
        cortex_loop(state).await?;
    }
    SlyMode::McpServer => {
        // NEW: Start MCP server
        let server = McpServer::new(vec![
            Box::new(AutonomousMcp),
            Box::new(KnowledgeMcp),
            Box::new(SafetyMcp),
        ]);
        server.serve().await?;
    }
    SlyMode::Init => init_workspace(false)?,
}
```

### Phase 4: **Discovery Integration**

Update `~/.sly/mcp/` to include a **self-reference**:

```bash
# ~/.sly/mcp/sly-server
#!/bin/bash
exec sly mcp-server
```

This allows Sly to **discover itself** as an MCP server when running in client mode, enabling **recursive tool composition**.

---

## Security Considerations

### 1. **Sandboxing**

When running as MCP server, enforce **OverlayFS by default**:

- All file operations go through safety shield
- No direct filesystem access without explicit `commit` tool call

### 2. **Resource Limits**

- `max_loops` circuit breaker on all autonomous tools
- Timeout enforcement (default: 5 minutes per tool call)
- Memory limits via ephemeral mode

### 3. **Audit Trail**

- All MCP tool calls logged to CozoDB
- Immutable event stream for forensics

---

## Benefits of Sly as MCP Server

### For External Agents

1. **Safety-Hardened Execution**: Leverage OverlayFS without implementing it
2. **Cross-Session Memory**: Access Sly's persistent knowledge graph
3. **Autonomous Capabilities**: Delegate complex multi-step tasks
4. **Rust Performance**: Native speed for compute-heavy operations

### For Sly

1. **Ecosystem Integration**: Works with any MCP client (Claude, Cursor, Zed)
2. **Composability**: External agents can chain Sly tools with their own
3. **Dogfooding**: Sly can consume its own MCP server (recursive improvement)
4. **Decomplection**: Clean separation of transport (MCP) from logic (Cortex)

---

## Roadmap Alignment

This proposal aligns with **Phase 5: Decomplecting I/O** in `ROADMAP.md`:

> "Eliminate hard-coded Telegram dependencies from the Core. Implementing a pure `Event` schema."

MCP server mode is the **ultimate I/O decomplection**:

- Core logic remains pure (Cortex, Memory, Safety)
- I/O becomes **pluggable** (Telegram, MCP, CLI, HTTP)
- The medium changes without recompiling the mind

---

## Next Steps

1. **Prototype**: Implement basic MCP server transport in `src/mcp/server.rs`
2. **Tool Migration**: Convert one existing tool (e.g., `sly_knowledge_search`) to MCP format
3. **Integration Test**: Connect Claude Desktop to Sly MCP server
4. **Iteration**: Expand tool surface based on real-world usage
5. **Documentation**: Create MCP server setup guide for users

---

## Conclusion

Sly as an MCP server represents the **natural evolution** of its architecture:

- **Simplicity**: One protocol, many clients
- **Decomplection**: Transport separated from logic
- **Composability**: Tools as data, not code
- **Reliability**: Safety guarantees extend to all consumers

This is not a pivot—it's a **generalization**. Sly becomes a **universal substrate** for autonomous coding intelligence, accessible to any agent that speaks MCP.

> "We can make the same exact software we are making today with dramatically simpler stuff — dramatically simpler languages, tools, techniques, approaches." — Rich Hickey

MCP is that simpler approach. Let's build it.
