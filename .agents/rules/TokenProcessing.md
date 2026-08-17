# Token Processing Pipeline

Dictates how text state is managed and processed to prevent context bloat.

## Core Directives
Defines how the system filters input streams efficiently.

## Lua Integration
Utilizes embedded Lua scripts (`scripts/filter.lua`) via the MLua engine to execute real-time context filtering. The engine handles zero-cost abstractions over the token data.

## Environment Variables
Adheres to `RTK_ENV` and `RTK_SCRIPT_PATH` for configuration injection.
- `RTK_ENV`: Specifies the environment (e.g., development, production).
- `RTK_SCRIPT_PATH`: Defines where the lua script lives (default: `scripts/filter.lua`).

## Pipeline Modules
Ensures memory-safe operations when parsing tokens within `TokenProcessor`.
