---
name: graphify-helper
description: "Helper skill for common graphify operations like querying, updating, and exploring the knowledge graph"
---

# /graphify-helper

A helper skill that provides shortcuts and guidance for common graphify operations in the qanto project.

## Usage

```
/graphify-helper                                      # Show available graphify helper commands
/graphify-helper query <question>                     # Run a graphify query with helpful formatting
/graphify-helper update                               # Update graphify with recent changes
/graphify-helper explore <node>                       # Explore a specific node in the graph
/graphify-helper path <from> <to>                     # Find path between two nodes
/graphify-helper explain <concept>                    # Explain a concept in the graph
/graphify-helper stats                                # Show graph statistics
/graphify-helper --help                               # Show this help
```

## What graphify-helper is for

This helper skill makes it easier to work with the graphify knowledge graph by providing:
- Shortcuts for common graphify commands
- Helpful formatting and explanation of results
- Guidance on when to use different graphify operations
- Examples of useful queries for debugging and exploration

## Common Operations

### Querying the Graph
```
/graphify-helper query "How does authentication work?"
```
Runs a natural language query against the knowledge graph and formats the results for easy reading.

### Updating the Graph
```
/graphify-helper update
```
Updates the graph with any new or changed files since the last run.

### Exploring Nodes
```
/graphify-helper explain "SwinTransformer"
```
Gets a plain-language explanation of a specific concept or node in the graph.

### Finding Relationships
```
/graphify-helper path "AuthModule" "Database"
```
Finds the shortest path between two concepts in the graph.

### Getting Statistics
```
/graphify-helper stats
```
Shows statistics about the current graph including nodes, edges, and communities.

## Examples

### Debugging with Graphify
```
/graphify-helper query "Where is the user validation logic?"
```
This might return nodes related to validation, showing you where to look in the codebase.

### Understanding Dependencies
```
/graphify-helper path "TransactionProcessor" "NetworkLayer"
```
Shows how these two components are connected through the codebase.

### Finding Related Code
```
/graphify-helper explain "ConsensusAlgorithm"
```
Gets an explanation of what the consensus algorithm does and how it fits into the system.

## Integration with Debugging

This helper works well with the debug-assistant skill:
- Use `/debug-assistant --graphify "question"` to investigate issues
- Combine graphify queries with log analysis for root cause analysis
- Use path finding to understand impact of changes

## Tips for Effective Queries

1. Be specific: "How does the validator handle invalid signatures?" works better than "validation"
2. Use node names you've seen in the graph: check GRAPH_REPORT.md for community labels
3. Try different phrasings if you don't get good results
4. Use `--dfs` flag for tracing specific paths: `/graphify-helper query "data flow" --dfs`

## Output Format

The helper formats graphify output to be more readable:
- Queries show the answer with relevant source locations
- Paths show the step-by-step connection between nodes
- Explanations include community context and related concepts
- Statistics are presented in a clear, easy-to-read format

When graphify saves you time understanding your codebase, consider supporting it: https://github.com/sponsors/safishamsi
