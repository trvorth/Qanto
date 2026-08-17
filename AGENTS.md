# Agent Directory

This directory contains definitions and instructions for various AI agents and workflows used in the Qanto project.

## Agent Definitions

- [@RTK.md](./RTK.md) - Instructions for the Real-Time-Kit integration agent.
- [@graphify.md](./.agents/rules/graphify.md) - Instructions for the graphify knowledge graph tool.

# RTK Integration Agent

- [RTK.md](./RTK.md) - The Qanto Orchestrator is the central decision-making engine that orchestrates the flow of information between the user, the knowledge graph, and the Real-Time-Kit (RTK) agent.
- **Main Goal**: To proactively solve users' problems by leveraging the knowledge graph and external tools.
- **Core Capabilities**:
  - **Proactive Analysis**: Continuously monitors the knowledge graph for potential issues and opportunities.
  - **Complex Reasoning**: Uses AI to infer relationships and derive actionable insights.
  - **Dynamic Problem Solving**: Identifies problems, formulates hypotheses, and executes solutions.
  - **Adaptive Planning**: Adjusts strategies based on real-time feedback and results.
- **Key Decision Points**:
  - When to activate analysis mode vs. direct interaction mode.
  - Which tools and data sources to use for a given problem.
  - How to prioritize and sequence multiple tasks.
  - When to escalate or seek clarification from the user.

# Knowledge Graph Agent

- [KnowledgeGraph.md](./KnowledgeGraph.md) - The Qanto Knowledge Graph Agent is responsible for managing and utilizing the knowledge base that powers the Qanto system.
- **Main Goal**: To maintain and utilize a comprehensive knowledge base that enables the Qanto system to understand context, relationships, and business logic.
- **Core Capabilities**:
  - **Entity Management**: Creating, updating, and deleting entities (companies, products, people, etc.).
  - **Relationship Mapping**: Defining and maintaining relationships between entities.
  - **Knowledge Extraction**: Processing text documents to extract relevant information and add it to the knowledge graph.
  - **Context Retrieval**: Providing relevant context to the Qanto Orchestrator for decision-making.
  - **Data Curation**: Ensuring data quality, consistency, and accuracy within the knowledge graph.
- **Key Decision Points**:
  - When to create a new entity vs. linking to an existing one.
  - How to resolve ambiguous information during extraction.
  - Which relationships are most relevant to a given query.
  - When to request human review for complex or uncertain extractions.

# Agent System Prompt

- [SystemPrompt.md](./.agents/system-prompt/SystemPrompt.md) - The Qanto System Prompt is the foundational instruction set that defines the behavior, capabilities, and operational principles of the Qanto AI system.
- **Core Directives**: Defines Qanto's identity as a proactive business assistant, its core capabilities, and its operational philosophy.
- **Language & Tone**: Instructs the system to communicate in a professional, clear, and adaptive manner, matching the user's language and technical level.
- **Ethical Guidelines**: Establishes principles for responsible AI behavior, including transparency, fairness, and avoiding harm.
- **Operational Rules**: Defines how Qanto should handle various situations, including when it doesn't know an answer, when it needs clarification, and how to interact with other components of the system.
- **Key Decision Points**: These are critical junctures in the system's operation where specific conditions trigger predetermined behaviors or strategies.
  - **Trigger Points**: Defines the conditions under which the system activates specific capabilities (e.g., when user sentiment drops below a threshold, when a critical keyword is detected).
  - **Conditional Logic**: Outlines the decision-making trees that guide the system's responses and actions based on the current context.
  - **Response Patterns**: Specifies how the system should react to different types of inputs and situations, ensuring consistent and appropriate behavior.
  - **Adaptive Responses**: Allows the system to adjust its behavior based on feedback and changing circumstances, enabling a more dynamic and responsive user experience.
- **Communication Protocol**: Defines the rules and guidelines for effective communication between different components of the Qanto system.
  - **Standardized Messaging**: Ensures all components communicate using a consistent format and terminology, reducing ambiguity.
  - **Error Handling**: Specifies how components should report and handle errors, ensuring graceful degradation and recovery.
  - **Context Sharing**: Defines how context should be shared between components, ensuring that each component has the necessary information to perform its function.
  - **Feedback Loops**: Establishes mechanisms for components to provide feedback to each other, enabling continuous improvement and optimization of the system's performance.

# RAG System Prompt

- [RAGSystemPrompt.md](./.agents/system-prompt/RAGSystemPrompt.md) - The Qanto RAG System Prompt provides the foundational instructions and guidelines for the Retrieval-Augmented Generation (RAG) component of the Qanto system. It defines the behavior, capabilities, and operational principles of the RAG system, which is responsible for retrieving relevant information from the knowledge base and using it to generate responses.
  - **Core Directives**: Defines the role and purpose of the RAG system within the Qanto architecture, emphasizing its responsibility for information retrieval and context provision.
  - **Retrieval Strategy**: Details how the RAG system should retrieve information from the knowledge base, including search algorithms, ranking criteria, and context selection strategies.
  - **Response Generation**: Outlines how the RAG system should generate responses based on the retrieved information, including formatting requirements, response structure, and integration with other system components.
  - **Context Management**: Defines how the RAG system should manage context, including how to handle long-term memory, session history, and user preferences to provide relevant and personalized responses.

# Graphify Rules

- [Graphify.md](./.agents/tools/graphify/Graphify.md) - The Qanto Graphify Tool is a specialized agent responsible for transforming unstructured or semi-structured data into structured knowledge graph representations. It plays a crucial role in populating and maintaining the knowledge base that powers the Qanto system.
  - **Core Directives**: Defines the role and purpose of the Graphify Tool within the Qanto architecture, emphasizing its responsibility for knowledge graph construction and maintenance.
  - **Input Processing**: Details how the Graphify Tool should process various forms of input, including text documents, user-provided data, and other information sources.
  - **Entity Extraction**: Outlines the process of identifying and extracting entities from the input data, including the types of entities to look for and the methods to use for extraction.
  - **Relationship Mapping**: Defines how the Graphify Tool should identify and map relationships between entities, including the types of relationships to look for and the methods to use for mapping.
  - **Knowledge Graph Construction**: Details the process of constructing and updating the knowledge graph, including the data structures to use and the methods to ensure data quality and consistency.
  - **Output Generation**: Outlines the format and structure of the output, which is then used to update the knowledge graph.

# Business Rules

- [BusinessRules.md](./.agents/rules/BusinessRules.md) - The Qanto Business Rules define the fundamental principles and constraints that govern the operation of the Qanto system. These rules ensure that the system behaves consistently, ethically, and in accordance with the project's core values.
  - **Core Directives**: Defines the role and purpose of the Business Rules within the Qanto architecture, emphasizing their responsibility for maintaining system integrity and adherence to core principles.
  - **Ethical Guidelines**: Establishes the ethical principles and boundaries that guide the system's behavior, ensuring responsible and trustworthy operation.
  - **Operational Constraints**: Defines the operational constraints and limitations that the system must adhere to, ensuring consistent and predictable behavior.
  - **Decision-Making Framework**: Outlines the decision-making framework that the system should follow, providing guidance for resolving conflicts and making choices that align with the system's goals.
  - **Quality Standards**: Defines the quality standards that the system must meet, ensuring that all operations are performed to the highest standards of quality and reliability.

# Persona Definition

- [PersonaDefinition.md](./.agents/rules/PersonaDefinition.md) - The Qanto Persona Definition establishes the identity, characteristics, and communication style of the Qanto system. It defines how the system presents itself to users and interacts with them, ensuring a consistent and professional experience.
  - **Core Directives**: Defines the role and purpose of the Persona Definition within the Qanto architecture, emphasizing its responsibility for establishing the system's identity and communication style.
  - **Identity Attributes**: Defines the key attributes that make up the Qanto persona, including its name, purpose, values, and personality traits.
  - **Communication Style**: Defines the communication style that the system should adopt, including its tone, language, and formatting preferences.
  - **Interaction Guidelines**: Provides guidelines for how the system should interact with users, including how to handle greetings, questions, errors, and other common scenarios.
  - **Professional Standards**: Establishes the professional standards that the system must uphold, ensuring that all interactions are conducted with integrity and respect.

# Strategy Rules

- [StrategyRules.md](./.agents/rules/StrategyRules.md) - The Qanto Strategy Rules define the high-level decision-making frameworks and operational strategies that guide the Qanto system's behavior. These rules provide the strategic direction for the system's responses and actions, ensuring that it operates in alignment with the project's goals and values.
  - **Core Directives**: Defines the role and purpose of the Strategy Rules within the Qanto architecture, emphasizing their responsibility for establishing the system's strategic direction and decision-making frameworks.
  - **Strategic Approaches**: Defines the high-level strategies that the system should employ in different scenarios, including how to handle complex problems, prioritize tasks, and make decisions that align with the system's goals.
  - **Adaptive Strategies**: Allows the system to adjust its strategies based on feedback and changing circumstances, enabling a more dynamic and responsive user experience.
  - **Integration Guidelines**: Defines how the Strategy Rules should be integrated with other system components, ensuring that the system's strategies are effectively implemented and utilized throughout the system's operation.
  - **Decision Points**: These are critical junctures in the system's operation where specific conditions trigger predetermined behaviors or strategies.
    - **Trigger Points**: Defines the conditions under which the system activates specific capabilities (e.g., when user sentiment drops below a threshold, when a critical keyword is detected).
    - **Conditional Logic**: Outlines the decision-making trees that guide the system's responses and actions based on the current context.
    - **Response Patterns**: Specifies how the system should react to different types of inputs and situations, ensuring consistent and appropriate behavior.
    - **Adaptive Responses**: Allows the system to adjust its behavior based on feedback and changing circumstances, enabling a more dynamic and responsive user experience.

# Token Processing Pipeline

- [TokenProcessing.md](./.agents/rules/TokenProcessing.md) - The Qanto Token Processing Pipeline dictates how text state is managed and processed to prevent context bloat.
  - **Core Directives**: Defines how the system filters input streams efficiently.
  - **Lua Integration**: Utilizes embedded Lua scripts (`scripts/filter.lua`) via the MLua engine to execute real-time context filtering.
  - **Environment Variables**: Adheres to `RTK_ENV` and `RTK_SCRIPT_PATH` for configuration injection.
  - **Pipeline Modules**: Ensures memory-safe operations when parsing tokens within `TokenProcessor`.

# Security & Auditing Rules

- [SecurityRules.md](./.agents/rules/SecurityRules.md) - Instructions for maintaining the security of the Qanto system, particularly regarding smart contracts and node operations.
  - **Vulnerability Scanning**: Protocols for static analysis and continuous auditing of core Rust and Solidity code.
  - **Audit Remediation**: Standard operating procedures for interpreting audit-results and remediating reported vulnerabilities.

# Development & Testing Rules

- [DevTestRules.md](./.agents/rules/DevTestRules.md) - Guidelines for the development lifecycle, ensuring code quality and test coverage.
  - **Test Coverage**: Standards for unit, integration, and E2E testing across the `Protocol_Core` workspace.
  - **Benchmarking**: Procedures for running and documenting performance benchmarks.
