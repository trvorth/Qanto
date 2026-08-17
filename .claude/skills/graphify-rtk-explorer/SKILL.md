---
name: graphify-rtk-explorer
description: "Cross-project skill that combines graphify knowledge graph exploration with RTK-optimized commands for efficient debugging and analysis"
---

# /graphify-rtk-explorer

A cross-project skill that helps you efficiently explore codebases by combining graphify knowledge graph queries with RTK token-optimized commands. This skill works in both the qanto project (which has graphify) and the saga project (which has RTK).

## Usage

```
/graphify-rtk-explorer                                      # Show available cross-project commands
/graphify-rtk-explorer debug <issue>                        # Debug an issue using both tools
/graphify-rtk-explorer explore <concept>                    # Explore a concept with graphify + RTK
/graphify-rtk-explorer trace <from> <to>                    # Trace path between concepts
/graphify-rtk-explorer analyze <area>                       # Analyze code area with both tools
/graphify-rtk-explorer optimize <command> <graph-query>     # Run RTK-optimized command based on graph results
/graphify-rtk-explorer --help                               # Show this help
```

## What graphify-rtk-explorer is for

This skill combines the strengths of both systems:
- **graphify** (qanto): Provides deep understanding of code relationships, architecture, and dependencies
- **RTK** (saga): Provides token-optimized commands for efficient execution and output

Together they enable:
- Fast, informed debugging with reduced token usage
- Efficient code exploration that leverages both structural knowledge and optimized commands
- Reduced context switching between understanding and execution
- Better use of limited context windows in AI-assisted development

## Combined Workflow

The skill follows this workflow when helping you analyze code:

### Phase 1 - Understand with graphify
First use graphify to understand the codebase structure:
- Query relationships between components
- Find paths between concepts
- Identify god nodes and surprising connections
- Get explanations of complex systems

### Phase 2 - Execute with RTK
Then use RTK-optimized commands to investigate:
- Run targeted tests with failure-only output
- Examine logs with deduplication
- Check specific files with compact output
- Filter error output from commands

### Phase 3 - Iterate
Use insights from RTK execution to refine graphify queries, and vice versa.

## Commands and Flags

### Debugging Workflow
```
/graphify-rtk-explorer debug "Authentication fails with timeout"
```
1. Uses graphify to find authentication-related components
2. Suggests RTK-optimized commands to test those components
3. Provides a streamlined debugging workflow

### Concept Exploration
```
/graphify-rtk-explorer explore "ConsensusAlgorithm"
```
1. Gets graphify explanation of the consensus algorithm
2. Shows related components and their relationships
3. Suggests RTK-optimized ways to examine the implementation

### Tracing Relationships
```
/graphify-rtk-explorer trace "Mempool" "BlockProducer"
```
1. Finds path between mempool and block producer in graph
2. Shows the connecting components and interfaces
3. Recommends RTK-optimized commands to verify each step

### Area Analysis
```
/graphify-rtk-explorer analyze "Networking layer"
```
1.Provides graphify overview of networking components
2. Shows community structure and key dependencies
3. Suggests RTK commands to inspect networking code efficiently

### Optimization Helper
```
/graphify-rtk-explorer optimize "cargo test" "networking components"
```
1. Uses graphify to find networking-related test targets
2. Returns RTK-optimized test command for those targets
3. Saves time by targeting only relevant tests

## Examples

### Debugging a Performance Issue
```
/graphify-rtk-explorer debug "Block validation is slow"
```
Workflow:
1. Graphify query: "What calls the block validation function?"
2. Get path from mempool to validation to storage
3. RTK suggestions: 
   - `rtk cargo test --bench block_validation` (benchmarks only)
   - `rtk log validation.log` (deduplicated logs)
   - `rtk err cargo test` (failure-only test output)

### Understanding a New Codebase
```
/graphify-rtk-explorer explore "TransactionHandling"
```
Workflow:
1. Graphify explain: Plain-language description of transaction handling
2. Graphify query: "What are the main components of transaction processing?"
3. RTK suggestions:
   - `rtk ls src/transactions/` (tree format directory listing)
   - `rtk read transaction.rs` (code reading with filtering)
   - `rtk grep "TransactionError"` (grouped search results)

### Preparing for Refactoring
```
/graphify-rtk-explorer analyze "Payment System"
```
Workflow:
1. Graphify stats: Nodes, edges, communities in payment system
2. Graphify surprising connections: Unexpected dependencies
3. RTK suggestions:
   - `rtk deps` (dependency overview focused on payment components)
   - `rtk summary cargo build` (smart summary of build output)
   - `rtk json Cargo.toml` (structure without values for config inspection)

## How It Works in Each Project

### In Qanto (has graphify)
- Full graphify query/explain/path functionality available
- RTK-optimized command suggestions provided
- Falls back to regular commands if RTK not available (but still suggests the pattern)

### In Saga (has RTK)
- RTK-optimized commands executed directly
- Graphify guidance provided based on general knowledge (if graphify-out exists)
- Can work with limited graphify functionality if graphify isn't installed

## Benefits of Combining Both Systems

### Token Efficiency
- Graphify reduces need to read large amounts of code to understand relationships
- RTK reduces output volume when executing commands
- Combined effect: Much less information to process in context windows

### Faster Iteration
- Quickly understand system with graphify
- Rapidly test hypotheses with RTK-optimized commands
- Tight feedback loop between understanding and verification

### Reduced Cognitive Load
- Graphify provides the "map" of the codebase
- RTK provides the "vehicles" to navigate it efficiently
- Less mental effort switching between modes

### Better Use of AI Assistance
- More context available for actual problem-solving
- Less context wasted on repetitive information
- AI can focus on higher-level reasoning rather than low-level details

## Implementation Tips

When using this skill:
1. Start with graphify to get your bearings
2. Use RTK commands to quickly verify assumptions
3. Let each iteration inform the next
4. Don't hesitate to run multiple lightweight RTK commands instead of one heavy one
5. Use the gain command periodically to see your savings

## Extending the Skill

You can enhance this cross-project skill by:
- Adding project-specific workflows (e.g., debugging consensus, optimizing networking)
- Incorporating team-specific patterns for using both tools together
- Adding templates for common investigation types
- Creating shortcuts for your team's most frequent combined operations

When you find effective ways to combine graphify exploration with RTK execution, consider adding them to this skill to make it even more powerful.
