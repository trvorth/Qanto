---
name: debug-assistant
description: "A debugging assistant that helps identify and resolve issues across codebases by analyzing logs, running tests, checking error patterns, and suggesting debugging commands. Works with both graphify (qanto) and RTK (saga) systems."
---

# /debug-assistant

A cross-project debugging assistant that helps you identify, analyze, and resolve issues in your codebase. This skill works with both the qanto project (which uses graphify for knowledge graph analysis) and the saga project (which uses RTK for token-optimized commands).

## Usage

```
/debug-assistant                                      # Start interactive debugging session
/debug-assistant <issue-description>                  # Debug a specific issue
/debug-assistant --logs                               # Analyze recent logs
/debug-assistant --test                               # Run tests and analyze failures
/debug-assistant --errors                             # Check for error patterns
/debug-assistant --suggest <command>                  # Get RTK-optimized version of a command
/debug-assistant --graphify <question>                # Use graphify to explore code relationships
/debug-assistant --rtk <command>                      # Run command with RTK optimization
/debug-assistant --help                               # Show this help
```

## What debug-assistant is for

When you encounter bugs, errors, or unexpected behavior, this skill helps you:
- Quickly gather relevant information (logs, test results, error messages)
- Analyze patterns and root causes
- Get optimized debugging commands using project-specific tools
- Explore code relationships to understand impact
- Suggest fixes based on common debugging patterns

## What You Must Do When Invoked

If the user invokes `/debug-assistant --help` or `/debug-assistant -h` (with no other arguments), print the contents of the `## Usage` section above verbatim and stop.

## Debugging Workflow

The debug-assistant follows this workflow when helping you debug:

### Step 1 - Information Gathering
Based on your issue description or flags, the assistant will:
- Collect recent logs from relevant sources
- Check test results if applicable
- Look for error patterns in output
- Use project-specific tools to gather context

### Step 2 - Analysis
The assistant will:
- Analyze collected information for patterns
- Identify potential root causes
- Check error messages against known issues
- Use graphify (in qanto) to explore code relationships when relevant
- Use RTK (in saga) to get optimized command suggestions

### Step 3 - Recommendations
The assistant will provide:
- Specific commands to run (optimized with RTK where applicable)
- Suggestions for further investigation
- Potential fixes based on analysis
- Questions to help narrow down the issue

## Commands and Flags

### Basic Usage
```
/debug-assistant "My app crashes when loading data"
```
Starts debugging with the given issue description.

### Information Gathering Flags
```
/debug-assistant --logs           # Show and analyze recent logs
/debug-assistant --test           # Run tests and focus on failures
/debug-assistant --errors         # Scan for error patterns in recent output
/debug-assistant --context        # Show relevant file context around errors
```

### Project-Specific Tools
```
/debug-assistant --graphify "How does authentication work?"  # Query graphify knowledge graph (qanto)
/debug-assistant --rtk "cargo test"                          # Run command with RTK optimization
/debug-assistant --suggest "git log --oneline"              # Get RTK-optimized version of command
```

### Analysis Flags
```
/debug-assistant --trace          # Suggest tracing approaches
/debug-assistant --profile        # Suggest profiling approaches
/debug-assistant --deps           # Check dependency issues
```

## Examples

### Debugging a Test Failure
```
/debug-assistant "TestAuth::test_login fails with timeout"
```
The assistant will:
1. Check test logs for the specific failure
2. Look for timeout patterns
3. Suggest using graphify to trace authentication-related code (qanto)
4. Recommend RTK-optimized commands for debugging

### Investigating Performance Issues
```
/debug-assistant --profile "API response time increased 3x"
```
The assistant will:
1. Suggest profiling tools appropriate for each project
2. Recommend checking recent changes that might affect performance
3. Provide RTK-optimized profiling commands
4. Use graphify to identify hot paths in the codebase (qanto)

### General Issue Investigation
```
/debug-assistant --logs --errors
```
Combines log analysis with error pattern matching to quickly identify issues.

## How It Works With Each Project

### In the Qanto Project (graphify)
- Uses `graphify query` to explore code relationships when debugging
- Can analyze the knowledge graph to find related components
- Helps trace data flows and dependency chains
- Works with the existing graphify-out/ directory if present

### In the Saga Project (RTK)
- Automatically applies RTK optimization to suggested commands
- Uses RTK commands like `rtk log`, `rtk test`, `rtk err` for reduced token output
- Helps leverage the token-optimized commands for efficient debugging

## Implementation Details

This skill works by:
1. Analyzing your issue description to determine what information is needed
2. Gathering relevant data using appropriate tools for each project
3. Applying analysis techniques to identify patterns
4. Generating actionable recommendations
5. Providing project-specific optimizations

The skill is designed to be lightweight and fast, focusing on getting you the information you need to resolve issues quickly.

## Extending the Skill

You can customize this debugging assistant by:
- Adding project-specific debugging commands
- Incorporating team-specific debugging patterns
- Integrating with your project's logging and monitoring systems
- Adding custom analysis rules for common issues in your codebase

When you find useful debugging patterns, consider adding them to this skill to make it even more helpful for your team.
