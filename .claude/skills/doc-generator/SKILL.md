---
name: doc-generator
description: "Skill for generating documentation across codebases, works with both graphify (qanto) and RTK (saga) projects"
---

# /doc-generator

A skill that helps generate documentation for your codebase by leveraging project-specific tools. Works with both the qanto project (which uses graphify for understanding code structure) and the saga project (which uses RTK for efficient command execution).

## Usage

```
/doc-generator                                      # Show documentation generation options
/doc-generator readme                               # Generate or update README.md
/doc-generator api                                  # Generate API documentation
/doc-generator <component>                          # Generate docs for specific component
/doc-generator --from-graph                         # Use graphify to inform documentation (qanto)
/doc-generator --rtk                                # Use RTK-optimized commands for doc generation (saga)
/doc-generator --help                               # Show this help
```

## What doc-generator is for

This skill helps you create and maintain documentation by:
- Leveraging code structure understanding (via graphify in qanto)
- Using efficient commands to extract information (via RTK in saga)
- Generating consistent, useful documentation formats
- Keeping documentation in sync with code changes

## What You Must Do When Invoked

If the user invokes `/doc-generator --help` or `/doc-generator -h` (with no other arguments), print the contents of the `## Usage` section above verbatim and stop.

## Documentation Generation Workflow

The doc-generator follows this workflow:

### Phase 1 - Gather Information
- Use graphify to understand component relationships (qanto)
- Use RTK-optimized commands to examine code efficiently (saga)
- Extract relevant information from source files, comments, and structure

### Phase 2 - Format Documentation
- Generate appropriate documentation format (README, API docs, etc.)
- Include examples, usage instructions, and explanations
- Structure information for readability and usefulness

### Phase 3 - Review and Refine
- Suggest areas that need more detail
- Recommend updates based on code changes
- Help maintain documentation accuracy

## Commands and Flags

### Basic Documentation
```
/doc-generator readme
```
Generates or updates README.md for the project with:
- Project overview
- Installation instructions
- Usage examples
- API reference (if applicable)
- Contribution guidelines

### API Documentation
```
/doc-generator api
```
Generates API documentation including:
- Function/method descriptions
- Parameter and return value explanations
- Usage examples
- Error conditions

### Component-Specific Docs
```
/doc-generator auth-service
```
Generates documentation for a specific component or service.

### Graphify-Informed Generation (Qanto)
```
/doc-generator --from-graph
```
Uses the graphify knowledge graph to:
- Identify important components to document
- Understand relationships between components
- Find god nodes and surprising connections worth highlighting
- Generate documentation that reflects actual code architecture

### RTK-Optimized Generation (Saga)
```
/doc-generator --rtk
```
Uses RTK-optimized commands to efficiently:
- Examine source code (`rtk read`, `rtk ls`)
- Search for patterns (`rtk grep`)
- Check file structures
- Extract information with minimal output

## Examples

### Generating Project Overview
```
/doc-generator readme --from-graph
```
In qanto, this would:
1. Use graphify query to find main components and architecture
2. Generate a README that accurately reflects the codebase structure
3. Include diagrams or descriptions of key relationships

### Creating API Docs
```
/doc-generator api --rtk
```
In saga, this would:
1. Use RTK-optimized commands to extract function signatures
2. Use `rtk read` to examine source code efficiently
3. Generate clear API documentation with examples

### Documenting a Specific Feature
```
/doc-generator consensus-algorithm
```
Would generate documentation specifically for the consensus algorithm component, including:
- Purpose and responsibility
- Key functions and data structures
- How it interacts with other components
- Usage examples if applicable

## How It Works in Each Project

### In Qanto Project (has graphify)
- Leverages graphify query to understand code structure before documenting
- Can identify which components are most important based on graph analysis
- Uses relationships in the graph to determine what to document together
- Falls back to source examination if graphify data isn't available

### In Saga Project (has RTK)
- Uses RTK-optimized commands to efficiently examine source material
- `rtk read` for code reading with filtering (60% savings)
- `rtk grep` for searching grouped by file (75% savings)
- `rtk ls` for tree format directory listings (65% savings)
- Combines information efficiently to create documentation

## Documentation Templates

The skill provides templates for different documentation types:

### README Template
```
# Project Name

## Overview
[Brief description of what the project does]

## Installation
[How to install and set up the project]

## Usage
[Examples of how to use the project]

## API Reference
[Link to or summary of API]

## Architecture
[Overview of project structure and key components]

## Contributing
[Guidelines for contributing to the project]

## License
[License information]
```

### API Documentation Template
```
# API Reference

## Module: [module_name]

### Function: [function_name]
[Description]
Parameters:
  - [param_name]: [type] - [description]
Returns: [type] - [description]
Errors: [List of possible errors and when they occur]
Example:
  [Code example showing usage]

## Class: [class_name]
[Description]
...
```

### Component Documentation Template
```
# [Component Name]

## Purpose
[What this component does and why it exists]

## Responsibilities
[List of key responsibilities]

## Key Functions/Methods
[Important functions and what they do]

## Data Structures
[Key data structures used]

## Dependencies
[Other components this one depends on]

## Dependents
[Components that depend on this one]

## Usage Example
[Code example showing how to use this component]
```

## Integration with Other Skills

This skill works well with:
- **debug-assistant**: Generate documentation while debugging to capture insights
- **graphify-helper**: Use graphify to inform what needs documentation
- **rtk-helper**: Use RTK commands to efficiently gather information
- **graphify-rtk-explorer**: Combine structural understanding with efficient execution

## Best Practices

1. **Keep it close to the code**: Documentation should live near the code it describes
2. **Update when code changes**: Make documentation updates part of your development workflow
3. **Focus on why, not just what**: Explain the reasoning behind design decisions
4. **Include examples**: Show how to use the code, not just what it does
5. **Be concise but complete**: Provide enough information to be useful without being overwhelming
6. **Use consistent formatting**: Follow established patterns in your project

When you find that this skill helps you create better documentation faster, consider how you might adapt it to your team's specific documentation needs and standards.
